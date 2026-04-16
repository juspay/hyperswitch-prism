# Adyen

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/adyen.json
Regenerate: python3 scripts/generators/docs/generate.py adyen
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
#     adyen=payment_pb2.AdyenConfig(api_key=...),
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
    connector: 'Adyen',
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
    .setConnector("Adyen")
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
    connector: "Adyen".to_string(),
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

**Examples:** [Python](../../examples/adyen/adyen.py#L362) · [JavaScript](../../examples/adyen/adyen.js) · [Kotlin](../../examples/adyen/adyen.kt#L120) · [Rust](../../examples/adyen/adyen.rs#L344)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/adyen/adyen.py#L381) · [JavaScript](../../examples/adyen/adyen.js) · [Kotlin](../../examples/adyen/adyen.kt#L136) · [Rust](../../examples/adyen/adyen.rs#L360)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/adyen/adyen.py#L406) · [JavaScript](../../examples/adyen/adyen.js) · [Kotlin](../../examples/adyen/adyen.kt#L158) · [Rust](../../examples/adyen/adyen.rs#L383)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/adyen/adyen.py#L431) · [JavaScript](../../examples/adyen/adyen.js) · [Kotlin](../../examples/adyen/adyen.kt#L180) · [Rust](../../examples/adyen/adyen.rs#L406)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [MerchantAuthenticationService.CreateClientAuthenticationToken](#merchantauthenticationservicecreateclientauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| [PaymentService.CreateOrder](#paymentservicecreateorder) | Payments | `PaymentServiceCreateOrderRequest` |
| [DisputeService.Accept](#disputeserviceaccept) | Disputes | `DisputeServiceAcceptRequest` |
| [DisputeService.Defend](#disputeservicedefend) | Disputes | `DisputeServiceDefendRequest` |
| [DisputeService.SubmitEvidence](#disputeservicesubmitevidence) | Disputes | `DisputeServiceSubmitEvidenceRequest` |
| [EventService.HandleEvent](#eventservicehandleevent) | Events | `EventServiceHandleRequest` |
| [PaymentService.IncrementalAuthorization](#paymentserviceincrementalauthorization) | Payments | `PaymentServiceIncrementalAuthorizationRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [PaymentService.ProxySetupRecurring](#paymentserviceproxysetuprecurring) | Payments | `PaymentServiceProxySetupRecurringRequest` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [PaymentService.SetupRecurring](#paymentservicesetuprecurring) | Payments | `PaymentServiceSetupRecurringRequest` |
| [PaymentService.TokenAuthorize](#paymentservicetokenauthorize) | Payments | `PaymentServiceTokenAuthorizeRequest` |
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
| Bancontact | ✓ |
| Apple Pay | ✓ |
| Apple Pay Dec | ✓ |
| Apple Pay SDK | ⚠ |
| Google Pay | ✓ |
| Google Pay Dec | ✓ |
| Google Pay SDK | ⚠ |
| PayPal SDK | ⚠ |
| Amazon Pay | ⚠ |
| Cash App | ⚠ |
| PayPal | ✓ |
| WeChat Pay | ⚠ |
| Alipay | ✓ |
| Revolut Pay | ⚠ |
| MiFinity | ⚠ |
| Bluecode | ⚠ |
| Paze | x |
| Samsung Pay | ⚠ |
| MB Way | ⚠ |
| Satispay | ⚠ |
| Wero | ⚠ |
| Affirm | ✓ |
| Afterpay | ✓ |
| Klarna | ✓ |
| UPI Collect | ⚠ |
| UPI Intent | ⚠ |
| UPI QR | ⚠ |
| Thailand | ✓ |
| Czech | ✓ |
| Finland | ✓ |
| FPX | ✓ |
| Poland | ⚠ |
| Slovakia | ✓ |
| UK | ✓ |
| PIS | x |
| Generic | ⚠ |
| Local | ⚠ |
| iDEAL | ✓ |
| Sofort | ⚠ |
| Trustly | ✓ |
| Giropay | ⚠ |
| EPS | ✓ |
| Przelewy24 | ⚠ |
| PSE | ⚠ |
| BLIK | ✓ |
| Interac | ⚠ |
| Bizum | ✓ |
| EFT | ⚠ |
| DuitNow | x |
| ACH | ⚠ |
| SEPA | ⚠ |
| BACS | ⚠ |
| Multibanco | ⚠ |
| Instant | ⚠ |
| Instant FI | ⚠ |
| Instant PL | ⚠ |
| Pix | ✓ |
| Permata | ✓ |
| BCA | ✓ |
| BNI VA | ✓ |
| BRI VA | ✓ |
| CIMB VA | ✓ |
| Danamon VA | ✓ |
| Mandiri VA | ✓ |
| Local | ⚠ |
| Indonesian | ⚠ |
| ACH | ✓ |
| SEPA | ✓ |
| BACS | ✓ |
| BECS | ⚠ |
| SEPA Guaranteed | ⚠ |
| Crypto | x |
| Reward | ⚠ |
| Givex | x |
| PaySafeCard | x |
| E-Voucher | ⚠ |
| Boleto | ✓ |
| Efecty | ⚠ |
| Pago Efectivo | ⚠ |
| Red Compra | ⚠ |
| Red Pagos | ⚠ |
| Alfamart | ✓ |
| Indomaret | ✓ |
| Oxxo | ✓ |
| 7-Eleven | ✓ |
| Lawson | ✓ |
| Mini Stop | ✓ |
| Family Mart | ✓ |
| Seicomart | ✓ |
| Pay Easy | ✓ |

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

##### BACS Direct Debit

```python
"payment_method": {
    "bacs": {  # Bacs - Bankers' Automated Clearing Services.
        "account_number": {"value": "55779911"},  # Account number for Bacs payment method.
        "sort_code": {"value": "200000"},  # Sort code for Bacs payment method.
        "bank_account_holder_name": {"value": "John Doe"}  # Holder name for bank debit.
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

##### iDEAL

```python
"payment_method": {
    "ideal": {
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

##### BLIK

```python
"payment_method": {
    "blik": {
        "blik_code": "777124"
    }
}
```

##### Klarna

```python
"payment_method": {
    "klarna": {  # Klarna - Swedish BNPL service.
    }
}
```

##### Afterpay / Clearpay

```python
"payment_method": {
    "afterpay_clearpay": {  # Afterpay/Clearpay - BNPL service.
    }
}
```

##### Affirm

```python
"payment_method": {
    "affirm": {  # Affirm - US BNPL service.
    }
}
```

**Examples:** [Python](../../examples/adyen/adyen.py#L453) · [TypeScript](../../examples/adyen/adyen.ts#L420) · [Kotlin](../../examples/adyen/adyen.kt#L198) · [Rust](../../examples/adyen/adyen.rs#L424)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/adyen/adyen.py#L462) · [TypeScript](../../examples/adyen/adyen.ts#L429) · [Kotlin](../../examples/adyen/adyen.kt#L210) · [Rust](../../examples/adyen/adyen.rs#L436)

#### PaymentService.CreateOrder

Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCreateOrderRequest` |
| **Response** | `PaymentServiceCreateOrderResponse` |

**Examples:** [Python](../../examples/adyen/adyen.py#L480) · [TypeScript](../../examples/adyen/adyen.ts#L447) · [Kotlin](../../examples/adyen/adyen.kt#L236) · [Rust](../../examples/adyen/adyen.rs#L450)

#### PaymentService.IncrementalAuthorization

Increase the authorized amount for an existing payment. Enables you to capture additional funds when the transaction amount changes after initial authorization.

| | Message |
|---|---------|
| **Request** | `PaymentServiceIncrementalAuthorizationRequest` |
| **Response** | `PaymentServiceIncrementalAuthorizationResponse` |

**Examples:** [Python](../../examples/adyen/adyen.py#L525) · [TypeScript](../../examples/adyen/adyen.ts#L492) · [Kotlin](../../examples/adyen/adyen.kt#L298) · [Rust](../../examples/adyen/adyen.rs#L485)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/adyen/adyen.py#L534) · [TypeScript](../../examples/adyen/adyen.ts#L501) · [Kotlin](../../examples/adyen/adyen.kt#L314) · [Rust](../../examples/adyen/adyen.rs#L492)

#### PaymentService.ProxySetupRecurring

Setup recurring mandate using vault-aliased card data.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxySetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/adyen/adyen.py#L543) · [TypeScript](../../examples/adyen/adyen.ts#L510) · [Kotlin](../../examples/adyen/adyen.kt#L355) · [Rust](../../examples/adyen/adyen.rs#L499)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/adyen/adyen.py#L561) · [TypeScript](../../examples/adyen/adyen.ts#L528) · [Kotlin](../../examples/adyen/adyen.kt#L434) · [Rust](../../examples/adyen/adyen.rs#L513)

#### PaymentService.SetupRecurring

Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/adyen/adyen.py#L570) · [TypeScript](../../examples/adyen/adyen.ts#L537) · [Kotlin](../../examples/adyen/adyen.kt#L444) · [Rust](../../examples/adyen/adyen.rs#L520)

#### PaymentService.TokenAuthorize

Authorize using a connector-issued payment method token.

| | Message |
|---|---------|
| **Request** | `PaymentServiceTokenAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/adyen/adyen.py#L579) · [TypeScript](../../examples/adyen/adyen.ts#L546) · [Kotlin](../../examples/adyen/adyen.kt#L499) · [Rust](../../examples/adyen/adyen.rs#L530)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/adyen/adyen.py#L588) · [TypeScript](../../examples/adyen/adyen.ts) · [Kotlin](../../examples/adyen/adyen.kt#L520) · [Rust](../../examples/adyen/adyen.rs#L537)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/adyen/adyen.py#L552) · [TypeScript](../../examples/adyen/adyen.ts#L519) · [Kotlin](../../examples/adyen/adyen.kt#L403) · [Rust](../../examples/adyen/adyen.rs#L506)

### Disputes

#### DisputeService.Accept

Concede dispute and accepts chargeback loss. Acknowledges liability and stops dispute defense process when evidence is insufficient.

| | Message |
|---|---------|
| **Request** | `DisputeServiceAcceptRequest` |
| **Response** | `DisputeServiceAcceptResponse` |

**Examples:** [Python](../../examples/adyen/adyen.py#L489) · [TypeScript](../../examples/adyen/adyen.ts#L456) · [Kotlin](../../examples/adyen/adyen.kt#L250) · [Rust](../../examples/adyen/adyen.rs#L457)

#### DisputeService.Defend

Submit defense with reason code for dispute. Presents formal argument against customer's chargeback claim with supporting documentation.

| | Message |
|---|---------|
| **Request** | `DisputeServiceDefendRequest` |
| **Response** | `DisputeServiceDefendResponse` |

**Examples:** [Python](../../examples/adyen/adyen.py#L498) · [TypeScript](../../examples/adyen/adyen.ts#L465) · [Kotlin](../../examples/adyen/adyen.kt#L262) · [Rust](../../examples/adyen/adyen.rs#L464)

#### DisputeService.SubmitEvidence

Upload evidence to dispute customer chargeback. Provides documentation like receipts and delivery proof to contest fraudulent transaction claims.

| | Message |
|---|---------|
| **Request** | `DisputeServiceSubmitEvidenceRequest` |
| **Response** | `DisputeServiceSubmitEvidenceResponse` |

**Examples:** [Python](../../examples/adyen/adyen.py#L507) · [TypeScript](../../examples/adyen/adyen.ts#L474) · [Kotlin](../../examples/adyen/adyen.kt#L275) · [Rust](../../examples/adyen/adyen.rs#L471)

### Authentication

#### MerchantAuthenticationService.CreateClientAuthenticationToken

Initialize client-facing SDK sessions for wallets, device fingerprinting, etc. Returns structured data the client SDK needs to render payment/verification UI.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/adyen/adyen.py#L471) · [TypeScript](../../examples/adyen/adyen.ts#L438) · [Kotlin](../../examples/adyen/adyen.kt#L220) · [Rust](../../examples/adyen/adyen.rs#L443)
