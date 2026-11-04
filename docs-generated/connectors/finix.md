# Finix

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/finix.json
Regenerate: python3 scripts/generators/docs/generate.py finix
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
        finix=payment_pb2.FinixConfig(
            finix_user_name=payment_methods_pb2.SecretString(value="YOUR_FINIX_USER_NAME"),
            finix_password=payment_methods_pb2.SecretString(value="YOUR_FINIX_PASSWORD"),
            merchant_identity_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_IDENTITY_ID"),
            merchant_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_ID"),
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
    connector: Connector.FINIX,
    environment: Environment.SANDBOX,
    auth: {
        finix: {
            finixUserName: { value: 'YOUR_FINIX_USER_NAME' },
            finixPassword: { value: 'YOUR_FINIX_PASSWORD' },
            merchantIdentityId: { value: 'YOUR_MERCHANT_IDENTITY_ID' },
            merchantId: { value: 'YOUR_MERCHANT_ID' },
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
            .setFinix(FinixConfig.newBuilder()
                .setFinixUserName(SecretString.newBuilder().setValue("YOUR_FINIX_USER_NAME").build())
                .setFinixPassword(SecretString.newBuilder().setValue("YOUR_FINIX_PASSWORD").build())
                .setMerchantIdentityId(SecretString.newBuilder().setValue("YOUR_MERCHANT_IDENTITY_ID").build())
                .setMerchantId(SecretString.newBuilder().setValue("YOUR_MERCHANT_ID").build())
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
            config: Some(connector_specific_config::Config::Finix(FinixConfig {
                finix_user_name: Some(hyperswitch_masking::Secret::new("YOUR_FINIX_USER_NAME".to_string())),  // Authentication credential
                finix_password: Some(hyperswitch_masking::Secret::new("YOUR_FINIX_PASSWORD".to_string())),  // Authentication credential
                merchant_identity_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_IDENTITY_ID".to_string())),  // Authentication credential
                merchant_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_ID".to_string())),  // Authentication credential
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

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentMethodAuthenticationService.Authenticate](#paymentmethodauthenticationserviceauthenticate) | Authentication | `PaymentMethodAuthenticationServiceAuthenticateRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [CustomerService.Create](#customerservicecreate) | Customers | `CustomerServiceCreateRequest` |
| [DisputeService.Accept](#disputeserviceaccept) | Disputes | `DisputeServiceAcceptRequest` |
| [DisputeService.Defend](#disputeservicedefend) | Disputes | `DisputeServiceDefendRequest` |
| [DisputeService.SubmitEvidence](#disputeservicesubmitevidence) | Disputes | `DisputeServiceSubmitEvidenceRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [EventService.HandleEvent](#eventservicehandleevent) | Events | `EventServiceHandleRequest` |
| [EventService.ParseEvent](#eventserviceparseevent) | Events | `EventServiceParseRequest` |
| [PaymentMethodAuthenticationService.PostAuthenticate](#paymentmethodauthenticationservicepostauthenticate) | Authentication | `PaymentMethodAuthenticationServicePostAuthenticateRequest` |
| [PaymentService.ProxySetupRecurring](#paymentserviceproxysetuprecurring) | Payments | `PaymentServiceProxySetupRecurringRequest` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.SetupRecurring](#paymentservicesetuprecurring) | Payments | `PaymentServiceSetupRecurringRequest` |
| [PaymentService.TokenAuthorize](#paymentservicetokenauthorize) | Payments | `PaymentServiceTokenAuthorizeRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Payments

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L301) · [Kotlin](../../examples/finix/finix.kt#L121) · [Rust](../../examples/finix/finix.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L346) · [Kotlin](../../examples/finix/finix.kt#L182) · [Rust](../../examples/finix/finix.rs)

#### PaymentService.ProxySetupRecurring

Setup recurring mandate using vault-aliased card data.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxySetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L382) · [Kotlin](../../examples/finix/finix.kt#L248) · [Rust](../../examples/finix/finix.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L400) · [Kotlin](../../examples/finix/finix.kt#L314) · [Rust](../../examples/finix/finix.rs)

#### PaymentService.SetupRecurring

Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L418) · [Kotlin](../../examples/finix/finix.kt#L336) · [Rust](../../examples/finix/finix.rs)

#### PaymentService.TokenAuthorize

Authorize using a connector-issued payment method token.

| | Message |
|---|---------|
| **Request** | `PaymentServiceTokenAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L427) · [Kotlin](../../examples/finix/finix.kt#L378) · [Rust](../../examples/finix/finix.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts) · [Kotlin](../../examples/finix/finix.kt#L399) · [Rust](../../examples/finix/finix.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L409) · [Kotlin](../../examples/finix/finix.kt#L324) · [Rust](../../examples/finix/finix.rs)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L391) · [Kotlin](../../examples/finix/finix.kt#L283) · [Rust](../../examples/finix/finix.rs)

### Customers

#### CustomerService.Create

Create customer record in the payment processor system. Stores customer details for future payment operations without re-sending personal information.

| | Message |
|---|---------|
| **Request** | `CustomerServiceCreateRequest` |
| **Response** | `CustomerServiceCreateResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L310) · [Kotlin](../../examples/finix/finix.kt#L131) · [Rust](../../examples/finix/finix.rs)

### Disputes

#### DisputeService.Accept

Concede dispute and accepts chargeback loss. Acknowledges liability and stops dispute defense process when evidence is insufficient.

| | Message |
|---|---------|
| **Request** | `DisputeServiceAcceptRequest` |
| **Response** | `DisputeServiceAcceptResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L319) · [Kotlin](../../examples/finix/finix.kt#L144) · [Rust](../../examples/finix/finix.rs)

#### DisputeService.Defend

Submit defense with reason code for dispute. Presents formal argument against customer's chargeback claim with supporting documentation.

| | Message |
|---|---------|
| **Request** | `DisputeServiceDefendRequest` |
| **Response** | `DisputeServiceDefendResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L328) · [Kotlin](../../examples/finix/finix.kt#L156) · [Rust](../../examples/finix/finix.rs)

#### DisputeService.SubmitEvidence

Upload evidence to dispute customer chargeback. Provides documentation like receipts and delivery proof to contest fraudulent transaction claims.

| | Message |
|---|---------|
| **Request** | `DisputeServiceSubmitEvidenceRequest` |
| **Response** | `DisputeServiceSubmitEvidenceResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L337) · [Kotlin](../../examples/finix/finix.kt#L169) · [Rust](../../examples/finix/finix.rs)

### Authentication

#### PaymentMethodAuthenticationService.Authenticate

Execute 3DS challenge or frictionless verification. Authenticates customer via bank challenge or behind-the-scenes verification for fraud prevention.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServiceAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServiceAuthenticateResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L292) · [Kotlin](../../examples/finix/finix.kt#L98) · [Rust](../../examples/finix/finix.rs)

#### PaymentMethodAuthenticationService.PostAuthenticate

Validate authentication results with the issuing bank. Processes bank's authentication decision to determine if payment can proceed.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServicePostAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServicePostAuthenticateResponse` |

**Examples:** [Python](../../examples/finix/finix.py) · [TypeScript](../../examples/finix/finix.ts#L373) · [Kotlin](../../examples/finix/finix.kt#L221) · [Rust](../../examples/finix/finix.rs)
