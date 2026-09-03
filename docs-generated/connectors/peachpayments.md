# Peachpayments

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/peachpayments.json
Regenerate: python3 scripts/generators/docs/generate.py peachpayments
-->

## SDK Configuration

Use this config for all flows in this connector. Replace the placeholders `YOUR_API_KEY`, `YOUR_TENANT_ID`, `YOUR_BASE_URL`, `YOUR_CLIENT_MERCHANT_REFERENCE_ID`, `YOUR_MERCHANT_PAYMENT_METHOD_ROUTE_ID` with your actual values.

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
        peachpayments=payment_pb2.PeachpaymentsConfig(
            api_key=payment_methods_pb2.SecretString(value="YOUR_API_KEY"),
            tenant_id=payment_methods_pb2.SecretString(value="YOUR_TENANT_ID"),
            base_url="YOUR_BASE_URL",
            client_merchant_reference_id=payment_methods_pb2.SecretString(value="YOUR_CLIENT_MERCHANT_REFERENCE_ID"),
            merchant_payment_method_route_id=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_PAYMENT_METHOD_ROUTE_ID"),
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
    connector: Connector.PEACHPAYMENTS,
    environment: Environment.SANDBOX,
    auth: {
        peachpayments: {
            apiKey: { value: 'YOUR_API_KEY' },
            tenantId: { value: 'YOUR_TENANT_ID' },
            baseUrl: 'YOUR_BASE_URL',
            clientMerchantReferenceId: { value: 'YOUR_CLIENT_MERCHANT_REFERENCE_ID' },
            merchantPaymentMethodRouteId: { value: 'YOUR_MERCHANT_PAYMENT_METHOD_ROUTE_ID' },
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
            .setPeachpayments(PeachpaymentsConfig.newBuilder()
                .setApiKey(SecretString.newBuilder().setValue("YOUR_API_KEY").build())
                .setTenantId(SecretString.newBuilder().setValue("YOUR_TENANT_ID").build())
                .setBaseUrl("YOUR_BASE_URL")
                .setClientMerchantReferenceId(SecretString.newBuilder().setValue("YOUR_CLIENT_MERCHANT_REFERENCE_ID").build())
                .setMerchantPaymentMethodRouteId(SecretString.newBuilder().setValue("YOUR_MERCHANT_PAYMENT_METHOD_ROUTE_ID").build())
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
            config: Some(connector_specific_config::Config::Peachpayments(PeachpaymentsConfig {
                api_key: Some(hyperswitch_masking::Secret::new("YOUR_API_KEY".to_string())),  // Authentication credential
                tenant_id: Some(hyperswitch_masking::Secret::new("YOUR_TENANT_ID".to_string())),  // Authentication credential
                base_url: Some("https://sandbox.example.com".to_string()),  // Base URL for API calls
                client_merchant_reference_id: Some(hyperswitch_masking::Secret::new("YOUR_CLIENT_MERCHANT_REFERENCE_ID".to_string())),  // Authentication credential
                merchant_payment_method_route_id: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_PAYMENT_METHOD_ROUTE_ID".to_string())),  // Authentication credential
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

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.py#L194) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.ts#L222) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.kt#L123) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.rs#L261)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.py#L213) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.ts#L241) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.kt#L139) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.rs#L277)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.py#L238) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.ts#L267) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.kt#L161) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.rs#L300)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.py#L263) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.ts#L293) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.kt#L183) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.rs#L323)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.py#L285) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.ts#L315) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.kt#L202) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.rs#L342)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [EventService.HandleEvent](#eventservicehandleevent) | Events | `EventServiceHandleRequest` |
| [EventService.ParseEvent](#eventserviceparseevent) | Events | `EventServiceParseRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [PaymentService.ProxySetupRecurring](#paymentserviceproxysetuprecurring) | Payments | `PaymentServiceProxySetupRecurringRequest` |
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
  "card": {
    "card_number": "4111111111111111",
    "card_exp_month": "03",
    "card_exp_year": "2030",
    "card_cvc": "737",
    "card_holder_name": "John Doe"
  }
}
```

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.ts#L336) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.kt#L220) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.ts#L345) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.kt#L232) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.ts#L354) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.kt#L242) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.ts#L381) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.kt#L281) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.rs)

#### PaymentService.ProxySetupRecurring

Setup recurring mandate using vault-aliased card data.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxySetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.ts#L390) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.kt#L310) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.ts#L399) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.kt#L342) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.rs)

#### PaymentService.SetupRecurring

Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.ts#L417) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.kt#L364) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.ts) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.kt#L403) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.ts#L408) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.kt#L352) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/peachpayments/peachpayments.rs)
