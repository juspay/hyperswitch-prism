# Novalnet

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/novalnet.json
Regenerate: python3 scripts/generators/docs/generate.py novalnet
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
#     novalnet=payment_pb2.NovalnetConfig(api_key=...),
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
    connector: 'Novalnet',
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
    .setConnector("Novalnet")
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
    connector: "Novalnet".to_string(),
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

**Examples:** [Python](../../examples/novalnet/novalnet.py#L273) · [JavaScript](../../examples/novalnet/novalnet.js) · [Kotlin](../../examples/novalnet/novalnet.kt#L118) · [Rust](../../examples/novalnet/novalnet.rs#L260)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/novalnet/novalnet.py#L292) · [JavaScript](../../examples/novalnet/novalnet.js) · [Kotlin](../../examples/novalnet/novalnet.kt#L134) · [Rust](../../examples/novalnet/novalnet.rs#L276)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/novalnet/novalnet.py#L317) · [JavaScript](../../examples/novalnet/novalnet.js) · [Kotlin](../../examples/novalnet/novalnet.kt#L156) · [Rust](../../examples/novalnet/novalnet.rs#L299)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/novalnet/novalnet.py#L342) · [JavaScript](../../examples/novalnet/novalnet.js) · [Kotlin](../../examples/novalnet/novalnet.kt#L178) · [Rust](../../examples/novalnet/novalnet.rs#L322)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/novalnet/novalnet.py#L364) · [JavaScript](../../examples/novalnet/novalnet.js) · [Kotlin](../../examples/novalnet/novalnet.kt#L197) · [Rust](../../examples/novalnet/novalnet.rs#L341)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [EventService.HandleEvent](#eventservicehandleevent) | Events | `EventServiceHandleRequest` |
| [PaymentService.IncrementalAuthorization](#paymentserviceincrementalauthorization) | Payments | `PaymentServiceIncrementalAuthorizationRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [PaymentService.ProxySetupRecurring](#paymentserviceproxysetuprecurring) | Payments | `PaymentServiceProxySetupRecurringRequest` |
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
| Bancontact | ⚠ |
| Apple Pay | ✓ |
| Apple Pay Dec | ? |
| Apple Pay SDK | ⚠ |
| Google Pay | ✓ |
| Google Pay Dec | ? |
| Google Pay SDK | ⚠ |
| PayPal SDK | ⚠ |
| Amazon Pay | ⚠ |
| Cash App | ⚠ |
| PayPal | ✓ |
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
| ACH | ✓ |
| SEPA | ✓ |
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

##### Google Pay

```python
"payment_method": {
    "google_pay": {  # Google Pay.
        "type": "CARD",  # Type of payment method.
        "description": "Visa 1111",  # User-facing description of the payment method.
        "info": {
            "card_network": "VISA",  # Card network name.
            "card_details": "1111"  # Card details (usually last 4 digits).
        },
        "tokenization_data": {
            "encrypted_data": {  # Encrypted Google Pay payment data.
                "token_type": "PAYMENT_GATEWAY",  # The type of the token.
                "token": "{\"id\":\"tok_probe_gpay\",\"object\":\"token\",\"type\":\"card\"}"  # Token generated for the wallet.
            }
        }
    }
}
```

##### Apple Pay

```python
"payment_method": {
    "apple_pay": {  # Apple Pay.
        "payment_data": {
            "encrypted_data": "eyJ2ZXJzaW9uIjoiRUNfdjEiLCJkYXRhIjoicHJvYmUiLCJzaWduYXR1cmUiOiJwcm9iZSJ9"  # Encrypted Apple Pay payment data as string.
        },
        "payment_method": {
            "display_name": "Visa 1111",
            "network": "Visa",
            "type": "debit"
        },
        "transaction_identifier": "probe_txn_id"  # Transaction identifier.
    }
}
```

##### SEPA Direct Debit

```python
"payment_method": {
    "sepa": {  # Sepa - Single Euro Payments Area direct debit.
        "iban": {"value": "DE89370400440532013000"},  # International bank account number (iban) for SEPA.
        "bank_account_holder_name": {"value": "John Doe"}  # Owner name for bank debit.
    }
}
```

##### ACH Direct Debit

```python
"payment_method": {
    "ach": {  # Ach - Automated Clearing House.
        "account_number": {"value": "000123456789"},  # Account number for ach bank debit payment.
        "routing_number": {"value": "110000000"},  # Routing number for ach bank debit payment.
        "bank_account_holder_name": {"value": "John Doe"}  # Bank account holder name.
    }
}
```

##### PayPal Redirect

```python
"payment_method": {
    "paypal_redirect": {  # PayPal.
        "email": {"value": "test@example.com"}  # PayPal's email address.
    }
}
```

**Examples:** [Python](../../examples/novalnet/novalnet.py#L386) · [TypeScript](../../examples/novalnet/novalnet.ts#L362) · [Kotlin](../../examples/novalnet/novalnet.kt#L215) · [Rust](../../examples/novalnet/novalnet.rs#L359)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/novalnet/novalnet.py#L395) · [TypeScript](../../examples/novalnet/novalnet.ts#L371) · [Kotlin](../../examples/novalnet/novalnet.kt#L227) · [Rust](../../examples/novalnet/novalnet.rs#L371)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/novalnet/novalnet.py#L404) · [TypeScript](../../examples/novalnet/novalnet.ts#L380) · [Kotlin](../../examples/novalnet/novalnet.kt#L237) · [Rust](../../examples/novalnet/novalnet.rs#L378)

#### PaymentService.IncrementalAuthorization

Increase the authorized amount for an existing payment. Enables you to capture additional funds when the transaction amount changes after initial authorization.

| | Message |
|---|---------|
| **Request** | `PaymentServiceIncrementalAuthorizationRequest` |
| **Response** | `PaymentServiceIncrementalAuthorizationResponse` |

**Examples:** [Python](../../examples/novalnet/novalnet.py#L422) · [TypeScript](../../examples/novalnet/novalnet.ts#L398) · [Kotlin](../../examples/novalnet/novalnet.kt#L255) · [Rust](../../examples/novalnet/novalnet.rs#L392)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/novalnet/novalnet.py#L431) · [TypeScript](../../examples/novalnet/novalnet.ts#L407) · [Kotlin](../../examples/novalnet/novalnet.kt#L271) · [Rust](../../examples/novalnet/novalnet.rs#L399)

#### PaymentService.ProxySetupRecurring

Setup recurring mandate using vault-aliased card data.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxySetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/novalnet/novalnet.py#L440) · [TypeScript](../../examples/novalnet/novalnet.ts#L416) · [Kotlin](../../examples/novalnet/novalnet.kt#L304) · [Rust](../../examples/novalnet/novalnet.rs#L406)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/novalnet/novalnet.py#L458) · [TypeScript](../../examples/novalnet/novalnet.ts#L434) · [Kotlin](../../examples/novalnet/novalnet.kt#L374) · [Rust](../../examples/novalnet/novalnet.rs#L420)

#### PaymentService.SetupRecurring

Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/novalnet/novalnet.py#L476) · [TypeScript](../../examples/novalnet/novalnet.ts#L452) · [Kotlin](../../examples/novalnet/novalnet.kt#L396) · [Rust](../../examples/novalnet/novalnet.rs#L434)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/novalnet/novalnet.py#L485) · [TypeScript](../../examples/novalnet/novalnet.ts) · [Kotlin](../../examples/novalnet/novalnet.kt#L440) · [Rust](../../examples/novalnet/novalnet.rs#L444)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/novalnet/novalnet.py#L467) · [TypeScript](../../examples/novalnet/novalnet.ts#L443) · [Kotlin](../../examples/novalnet/novalnet.kt#L384) · [Rust](../../examples/novalnet/novalnet.rs#L427)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/novalnet/novalnet.py#L449) · [TypeScript](../../examples/novalnet/novalnet.ts#L425) · [Kotlin](../../examples/novalnet/novalnet.kt#L341) · [Rust](../../examples/novalnet/novalnet.rs#L413)
