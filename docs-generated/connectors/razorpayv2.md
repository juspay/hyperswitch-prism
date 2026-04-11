# Razorpay V2

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/razorpayv2.json
Regenerate: python3 scripts/generators/docs/generate.py razorpayv2
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
#     razorpayv2=payment_pb2.Razorpayv2Config(api_key=...),
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
    connector: 'Razorpayv2',
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
    .setConnector("Razorpayv2")
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
    connector: "Razorpayv2".to_string(),
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

**Examples:** [Python](../../examples/razorpayv2/razorpayv2.py#L213) · [JavaScript](../../examples/razorpayv2/razorpayv2.js) · [Kotlin](../../examples/razorpayv2/razorpayv2.kt#L92) · [Rust](../../examples/razorpayv2/razorpayv2.rs#L206)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/razorpayv2/razorpayv2.py#L232) · [JavaScript](../../examples/razorpayv2/razorpayv2.js) · [Kotlin](../../examples/razorpayv2/razorpayv2.kt#L108) · [Rust](../../examples/razorpayv2/razorpayv2.rs#L222)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/razorpayv2/razorpayv2.py#L257) · [JavaScript](../../examples/razorpayv2/razorpayv2.js) · [Kotlin](../../examples/razorpayv2/razorpayv2.kt#L130) · [Rust](../../examples/razorpayv2/razorpayv2.rs#L245)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.CreateOrder](#paymentservicecreateorder) | Payments | `PaymentServiceCreateOrderRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.SetupRecurring](#paymentservicesetuprecurring) | Payments | `PaymentServiceSetupRecurringRequest` |
| [PaymentService.TokenAuthorize](#paymentservicetokenauthorize) | Payments | `PaymentServiceTokenAuthorizeRequest` |

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
| Apple Pay SDK | ✓ |
| Google Pay | ✓ |
| Google Pay Dec | ✓ |
| Google Pay SDK | ✓ |
| PayPal SDK | ✓ |
| Amazon Pay | ✓ |
| Cash App | ✓ |
| PayPal | ✓ |
| WeChat Pay | ✓ |
| Alipay | ✓ |
| Revolut Pay | ✓ |
| MiFinity | ✓ |
| Bluecode | ✓ |
| Paze | x |
| Samsung Pay | ✓ |
| MB Way | ✓ |
| Satispay | ✓ |
| Wero | ✓ |
| Affirm | ✓ |
| Afterpay | ✓ |
| Klarna | ✓ |
| UPI Collect | ✓ |
| UPI Intent | ✓ |
| UPI QR | ✓ |
| Thailand | ✓ |
| Czech | ✓ |
| Finland | ✓ |
| FPX | ✓ |
| Poland | ✓ |
| Slovakia | ✓ |
| UK | ✓ |
| PIS | x |
| Generic | ✓ |
| Local | ✓ |
| iDEAL | ✓ |
| Sofort | ✓ |
| Trustly | ✓ |
| Giropay | ✓ |
| EPS | ✓ |
| Przelewy24 | ✓ |
| PSE | ✓ |
| BLIK | ✓ |
| Interac | ✓ |
| Bizum | ✓ |
| EFT | ✓ |
| DuitNow | x |
| ACH | ✓ |
| SEPA | ✓ |
| BACS | ✓ |
| Multibanco | ✓ |
| Instant | ✓ |
| Instant FI | ✓ |
| Instant PL | ✓ |
| Pix | ✓ |
| Permata | ✓ |
| BCA | ✓ |
| BNI VA | ✓ |
| BRI VA | ✓ |
| CIMB VA | ✓ |
| Danamon VA | ✓ |
| Mandiri VA | ✓ |
| Local | ✓ |
| Indonesian | ✓ |
| ACH | ✓ |
| SEPA | ✓ |
| BACS | ✓ |
| BECS | ✓ |
| SEPA Guaranteed | ✓ |
| Crypto | x |
| Reward | ✓ |
| Givex | x |
| PaySafeCard | x |
| E-Voucher | ✓ |
| Boleto | ✓ |
| Efecty | ✓ |
| Pago Efectivo | ✓ |
| Red Compra | ✓ |
| Red Pagos | ✓ |
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

##### BECS Direct Debit

```python
"payment_method": {
    "becs": {  # Becs - Bulk Electronic Clearing System - Australian direct debit.
        "account_number": {"value": "000123456"},  # Account number for Becs payment method.
        "bsb_number": {"value": "000000"},  # Bank-State-Branch (bsb) number.
        "bank_account_holder_name": {"value": "John Doe"}  # Owner name for bank debit.
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

##### UPI Collect

```python
"payment_method": {
    "upi_collect": {  # UPI Collect.
        "vpa_id": {"value": "test@upi"}  # Virtual Payment Address.
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

##### Samsung Pay

```python
"payment_method": {
    "samsung_pay": {  # Samsung.
        "payment_credential": {
            "method": "3DS",  # Method type.
            "recurring_payment": False,  # Whether this is a recurring payment.
            "card_brand": "VISA",
            "card_last_four_digits": "1234",  # Last four digits of card.
            "token_data": {
                "type": "S",  # 3DS type.
                "version": "100",  # 3DS version.
                "data": "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6InNhbXN1bmdfcHJvYmVfa2V5XzEyMyJ9.eyJwYXltZW50TWV0aG9kVG9rZW4iOiJwcm9iZV9zYW1zdW5nX3Rva2VuIn0.ZHVtbXlfc2lnbmF0dXJl"  # Token data.
            }
        }
    }
}
```

**Examples:** [Python](../../examples/razorpayv2/razorpayv2.py#L279) · [TypeScript](../../examples/razorpayv2/razorpayv2.ts#L261) · [Kotlin](../../examples/razorpayv2/razorpayv2.kt#L148) · [Rust](../../examples/razorpayv2/razorpayv2.rs#L263)

#### PaymentService.CreateOrder

Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCreateOrderRequest` |
| **Response** | `PaymentServiceCreateOrderResponse` |

**Examples:** [Python](../../examples/razorpayv2/razorpayv2.py#L288) · [TypeScript](../../examples/razorpayv2/razorpayv2.ts#L270) · [Kotlin](../../examples/razorpayv2/razorpayv2.kt#L160) · [Rust](../../examples/razorpayv2/razorpayv2.rs#L275)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/razorpayv2/razorpayv2.py#L297) · [TypeScript](../../examples/razorpayv2/razorpayv2.ts#L279) · [Kotlin](../../examples/razorpayv2/razorpayv2.kt#L174) · [Rust](../../examples/razorpayv2/razorpayv2.rs#L282)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/razorpayv2/razorpayv2.py#L306) · [TypeScript](../../examples/razorpayv2/razorpayv2.ts#L288) · [Kotlin](../../examples/razorpayv2/razorpayv2.kt#L182) · [Rust](../../examples/razorpayv2/razorpayv2.rs#L289)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/razorpayv2/razorpayv2.py#L324) · [TypeScript](../../examples/razorpayv2/razorpayv2.ts#L306) · [Kotlin](../../examples/razorpayv2/razorpayv2.kt#L243) · [Rust](../../examples/razorpayv2/razorpayv2.rs#L303)

#### PaymentService.SetupRecurring

Configure a payment method for recurring billing. Sets up the mandate and payment details needed for future automated charges.

| | Message |
|---|---------|
| **Request** | `PaymentServiceSetupRecurringRequest` |
| **Response** | `PaymentServiceSetupRecurringResponse` |

**Examples:** [Python](../../examples/razorpayv2/razorpayv2.py#L342) · [TypeScript](../../examples/razorpayv2/razorpayv2.ts#L324) · [Kotlin](../../examples/razorpayv2/razorpayv2.kt#L265) · [Rust](../../examples/razorpayv2/razorpayv2.rs#L317)

#### PaymentService.TokenAuthorize

Authorize using a connector-issued payment method token.

| | Message |
|---|---------|
| **Request** | `PaymentServiceTokenAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/razorpayv2/razorpayv2.py#L351) · [TypeScript](../../examples/razorpayv2/razorpayv2.ts#L333) · [Kotlin](../../examples/razorpayv2/razorpayv2.kt#L305) · [Rust](../../examples/razorpayv2/razorpayv2.rs#L327)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/razorpayv2/razorpayv2.py#L333) · [TypeScript](../../examples/razorpayv2/razorpayv2.ts#L315) · [Kotlin](../../examples/razorpayv2/razorpayv2.kt#L253) · [Rust](../../examples/razorpayv2/razorpayv2.rs#L310)

### Mandates

#### RecurringPaymentService.Charge

Charge using an existing stored recurring payment instruction. Processes repeat payments for subscriptions or recurring billing without collecting payment details.

| | Message |
|---|---------|
| **Request** | `RecurringPaymentServiceChargeRequest` |
| **Response** | `RecurringPaymentServiceChargeResponse` |

**Examples:** [Python](../../examples/razorpayv2/razorpayv2.py#L315) · [TypeScript](../../examples/razorpayv2/razorpayv2.ts#L297) · [Kotlin](../../examples/razorpayv2/razorpayv2.kt#L211) · [Rust](../../examples/razorpayv2/razorpayv2.rs#L296)
