# Paysafe

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/paysafe.json
Regenerate: python3 scripts/generators/docs/generate.py paysafe
-->

## SDK Configuration

Use this config for all flows in this connector. Replace `YOUR_API_KEY` with your actual credentials.

<table>
<tr><td><b>Python</b></td><td><b>JavaScript</b></td><td><b>Kotlin</b></td><td><b>Rust</b></td></tr>
<tr>
<td valign="top">

<details><summary>Python</summary>

```python
from payments.generated import sdk_config_pb2, payment_pb2

config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
# Set credentials before running (field names depend on connector auth type):
# config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     paysafe=payment_pb2.PaysafeConfig(api_key=...),
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
    connector: 'Paysafe',
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
    .setConnector("Paysafe")
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
    connector: "Paysafe".to_string(),
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

**Examples:** [Python](../../examples/paysafe/paysafe.py#L156) · [JavaScript](../../examples/paysafe/paysafe.js) · [Kotlin](../../examples/paysafe/paysafe.kt#L110) · [Rust](../../examples/paysafe/paysafe.rs#L147)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/paysafe/paysafe.py#L175) · [JavaScript](../../examples/paysafe/paysafe.js) · [Kotlin](../../examples/paysafe/paysafe.kt#L126) · [Rust](../../examples/paysafe/paysafe.rs#L163)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/paysafe/paysafe.py#L200) · [JavaScript](../../examples/paysafe/paysafe.js) · [Kotlin](../../examples/paysafe/paysafe.kt#L148) · [Rust](../../examples/paysafe/paysafe.rs#L186)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/paysafe/paysafe.py#L225) · [JavaScript](../../examples/paysafe/paysafe.js) · [Kotlin](../../examples/paysafe/paysafe.kt#L170) · [Rust](../../examples/paysafe/paysafe.rs#L209)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/paysafe/paysafe.py#L247) · [JavaScript](../../examples/paysafe/paysafe.js) · [Kotlin](../../examples/paysafe/paysafe.kt#L189) · [Rust](../../examples/paysafe/paysafe.rs#L228)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.CreateOrder](#paymentservicecreateorder) | Payments | `PaymentServiceCreateOrderRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentMethodService.Tokenize](#paymentmethodservicetokenize) | Payments | `PaymentMethodServiceTokenizeRequest` |
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
            "card_last_four_digits": {"value": "1234"},  # Last four digits of card.
            "token_data": {
                "type": "S",  # 3DS type.
                "version": "100",  # 3DS version.
                "data": {"value": "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6InNhbXN1bmdfcHJvYmVfa2V5XzEyMyJ9.eyJwYXltZW50TWV0aG9kVG9rZW4iOiJwcm9iZV9zYW1zdW5nX3Rva2VuIn0.ZHVtbXlfc2lnbmF0dXJl"}  # Token data.
            }
        }
    }
}
```

**Examples:** [Python](../../examples/paysafe/paysafe.py#L269) · [TypeScript](../../examples/paysafe/paysafe.ts#L254) · [Kotlin](../../examples/paysafe/paysafe.kt#L207) · [Rust](../../examples/paysafe/paysafe.rs#L246)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/paysafe/paysafe.py#L278) · [TypeScript](../../examples/paysafe/paysafe.ts#L263) · [Kotlin](../../examples/paysafe/paysafe.kt#L219) · [Rust](../../examples/paysafe/paysafe.rs#L258)

#### PaymentService.CreateOrder

Create a payment order for later processing. Establishes a transaction context that can be authorized or captured in subsequent API calls.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCreateOrderRequest` |
| **Response** | `PaymentServiceCreateOrderResponse` |

**Examples:** [Python](../../examples/paysafe/paysafe.py#L287) · [TypeScript](../../examples/paysafe/paysafe.ts#L272) · [Kotlin](../../examples/paysafe/paysafe.kt#L229) · [Rust](../../examples/paysafe/paysafe.rs#L265)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/paysafe/paysafe.py#L296) · [TypeScript](../../examples/paysafe/paysafe.ts#L281) · [Kotlin](../../examples/paysafe/paysafe.kt#L243) · [Rust](../../examples/paysafe/paysafe.rs#L272)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/paysafe/paysafe.py#L305) · [TypeScript](../../examples/paysafe/paysafe.ts#L290) · [Kotlin](../../examples/paysafe/paysafe.kt#L251) · [Rust](../../examples/paysafe/paysafe.rs#L279)

#### PaymentMethodService.Tokenize

Tokenize payment method for secure storage. Replaces raw card details with secure token for one-click payments and recurring billing.

| | Message |
|---|---------|
| **Request** | `PaymentMethodServiceTokenizeRequest` |
| **Response** | `PaymentMethodServiceTokenizeResponse` |

**Examples:** [Python](../../examples/paysafe/paysafe.py#L323) · [TypeScript](../../examples/paysafe/paysafe.ts#L308) · [Kotlin](../../examples/paysafe/paysafe.kt#L273) · [Rust](../../examples/paysafe/paysafe.rs#L293)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/paysafe/paysafe.py#L332) · [TypeScript](../../examples/paysafe/paysafe.ts) · [Kotlin](../../examples/paysafe/paysafe.kt#L300) · [Rust](../../examples/paysafe/paysafe.rs#L300)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/paysafe/paysafe.py#L314) · [TypeScript](../../examples/paysafe/paysafe.ts#L299) · [Kotlin](../../examples/paysafe/paysafe.kt#L261) · [Rust](../../examples/paysafe/paysafe.rs#L286)
