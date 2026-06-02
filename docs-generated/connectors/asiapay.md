# Asiapay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/asiapay.json
Regenerate: python3 scripts/generators/docs/generate.py asiapay
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
    # connector_config=payment_pb2.ConnectorSpecificConfig(
    #     asiapay=payment_pb2.AsiapayConfig(api_key=...),
    # ),
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
    connector: Connector.ASIAPAY,
    environment: Environment.SANDBOX,
    // auth: { asiapay: { apiKey: { value: 'YOUR_API_KEY' } } },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    // .setConnectorConfig(...) — set your Asiapay credentials here
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
    connector_config: None,  // TODO: Add your connector config here,
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

**Examples:** [Python](../../examples/asiapay/asiapay.py#L143) · [JavaScript](../../examples/asiapay/asiapay.js) · [Kotlin](../../examples/asiapay/asiapay.kt#L104) · [Rust](../../examples/asiapay/asiapay.rs#L196)

### Card Payment (Authorize + Capture)

Two-step card payment. First authorize, then capture. Use when you need to verify funds before finalizing.

**Response status handling:**

| Status | Recommended action |
|--------|-------------------|
| `AUTHORIZED` | Funds reserved — proceed to Capture to settle |
| `PENDING` | Awaiting async confirmation — wait for webhook before capturing |
| `FAILED` | Payment declined — surface error to customer, do not retry without new details |

**Examples:** [Python](../../examples/asiapay/asiapay.py#L162) · [JavaScript](../../examples/asiapay/asiapay.js) · [Kotlin](../../examples/asiapay/asiapay.kt#L120) · [Rust](../../examples/asiapay/asiapay.rs#L212)

### Refund

Return funds to the customer for a completed payment.

**Examples:** [Python](../../examples/asiapay/asiapay.py#L187) · [JavaScript](../../examples/asiapay/asiapay.js) · [Kotlin](../../examples/asiapay/asiapay.kt#L142) · [Rust](../../examples/asiapay/asiapay.rs#L235)

### Void Payment

Cancel an authorized but not-yet-captured payment.

**Examples:** [Python](../../examples/asiapay/asiapay.py#L212) · [JavaScript](../../examples/asiapay/asiapay.js) · [Kotlin](../../examples/asiapay/asiapay.kt#L164) · [Rust](../../examples/asiapay/asiapay.rs#L258)

### Get Payment Status

Retrieve current payment status from the connector.

**Examples:** [Python](../../examples/asiapay/asiapay.py#L234) · [JavaScript](../../examples/asiapay/asiapay.js) · [Kotlin](../../examples/asiapay/asiapay.kt#L183) · [Rust](../../examples/asiapay/asiapay.rs#L277)

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [EventService.HandleEvent](#eventservicehandleevent) | Events | `EventServiceHandleRequest` |
| [EventService.ParseEvent](#eventserviceparseevent) | Events | `EventServiceParseRequest` |
| [PaymentService.ProxyAuthorize](#paymentserviceproxyauthorize) | Payments | `PaymentServiceProxyAuthorizeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
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
| GoPay | ✓ |
| GCash | ✓ |
| Momo | ✓ |
| Dana | ✓ |
| Kakao Pay | ✓ |
| Touch 'n Go | ✓ |
| Twint | ✓ |
| Vipps | ✓ |
| Swish | ✓ |
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
      "encrypted_data": "eyJ2ZXJzaW9uIjoiRUNfdjEiLCJkYXRhIjoicHJvYmUiLCJzaWduYXR1cmUiOiJwcm9iZSJ9"
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

##### SEPA Direct Debit

```python
"payment_method": {
  "sepa": {
    "iban": "DE89370400440532013000",
    "bank_account_holder_name": "John Doe"
  }
}
```

##### BACS Direct Debit

```python
"payment_method": {
  "bacs": {
    "account_number": "55779911",
    "sort_code": "200000",
    "bank_account_holder_name": "John Doe"
  }
}
```

##### ACH Direct Debit

```python
"payment_method": {
  "ach": {
    "account_number": "000123456789",
    "routing_number": "110000000",
    "bank_account_holder_name": "John Doe"
  }
}
```

##### BECS Direct Debit

```python
"payment_method": {
  "becs": {
    "account_number": "000123456",
    "bsb_number": "000000",
    "bank_account_holder_name": "John Doe"
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
  "klarna": {}
}
```

##### Afterpay / Clearpay

```python
"payment_method": {
  "afterpay_clearpay": {}
}
```

##### UPI Collect

```python
"payment_method": {
  "upi_collect": {
    "vpa_id": "test@upi"
  }
}
```

##### Affirm

```python
"payment_method": {
  "affirm": {}
}
```

##### Samsung Pay

```python
"payment_method": {
  "samsung_pay_sdk": {
    "payment_credential": {
      "method": "3DS",
      "recurring_payment": false,
      "card_brand": "VISA",
      "card_last_four_digits": "1234",
      "token_data": {
        "type": "S",
        "version": "100",
        "data": "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6InNhbXN1bmdfcHJvYmVfa2V5XzEyMyJ9.eyJwYXltZW50TWV0aG9kVG9rZW4iOiJwcm9iZV9zYW1zdW5nX3Rva2VuIn0.ZHVtbXlfc2lnbmF0dXJl"
      }
    }
  }
}
```

**Examples:** [Python](../../examples/asiapay/asiapay.py) · [TypeScript](../../examples/asiapay/asiapay.ts#L281) · [Kotlin](../../examples/asiapay/asiapay.kt#L201) · [Rust](../../examples/asiapay/asiapay.rs)

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](../../examples/asiapay/asiapay.py) · [TypeScript](../../examples/asiapay/asiapay.ts#L290) · [Kotlin](../../examples/asiapay/asiapay.kt#L213) · [Rust](../../examples/asiapay/asiapay.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](../../examples/asiapay/asiapay.py) · [TypeScript](../../examples/asiapay/asiapay.ts#L299) · [Kotlin](../../examples/asiapay/asiapay.kt#L223) · [Rust](../../examples/asiapay/asiapay.rs)

#### PaymentService.ProxyAuthorize

Authorize using vault-aliased card data. Proxy substitutes before connector.

| | Message |
|---|---------|
| **Request** | `PaymentServiceProxyAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/asiapay/asiapay.py) · [TypeScript](../../examples/asiapay/asiapay.ts#L326) · [Kotlin](../../examples/asiapay/asiapay.kt#L262) · [Rust](../../examples/asiapay/asiapay.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/asiapay/asiapay.py) · [TypeScript](../../examples/asiapay/asiapay.ts#L335) · [Kotlin](../../examples/asiapay/asiapay.kt#L291) · [Rust](../../examples/asiapay/asiapay.rs)

#### PaymentService.TokenAuthorize

Authorize using a connector-issued payment method token.

| | Message |
|---|---------|
| **Request** | `PaymentServiceTokenAuthorizeRequest` |
| **Response** | `PaymentServiceAuthorizeResponse` |

**Examples:** [Python](../../examples/asiapay/asiapay.py) · [TypeScript](../../examples/asiapay/asiapay.ts#L353) · [Kotlin](../../examples/asiapay/asiapay.kt#L313) · [Rust](../../examples/asiapay/asiapay.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/asiapay/asiapay.py) · [TypeScript](../../examples/asiapay/asiapay.ts) · [Kotlin](../../examples/asiapay/asiapay.kt#L334) · [Rust](../../examples/asiapay/asiapay.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/asiapay/asiapay.py) · [TypeScript](../../examples/asiapay/asiapay.ts#L344) · [Kotlin](../../examples/asiapay/asiapay.kt#L301) · [Rust](../../examples/asiapay/asiapay.rs)
