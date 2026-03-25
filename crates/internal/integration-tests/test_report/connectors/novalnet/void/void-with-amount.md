# Connector `novalnet` / Suite `void` / Scenario `void_with_amount`

- Service: `PaymentService/Void`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

**Pre Requisites Executed**

<details>
<summary>1. create_customer(create_customer) — FAIL</summary>

**Dependency Error**

```text
sdk call failed: sdk HTTP request failed for 'create_customer'/'create_customer': builder error
```

<details>
<summary>Show Dependency Request (masked)</summary>

_Request trace not available._

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

_Response trace not available._

</details>

</details>
<details>
<summary>2. authorize(no3ds_manual_capture_credit_card) — FAIL</summary>

**Dependency Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

<details>
<summary>Show Dependency Request (masked)</summary>

```json
{
  "merchant_transaction_id": "mti_e80b658826e343cc8c941a672a1d98fd",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "card": {
      "card_number": "***MASKED***",
      "card_exp_month": {
        "value": "08"
      },
      "card_exp_year": {
        "value": "30"
      },
      "card_cvc": "***MASKED***",
      "card_holder_name": {
        "value": "Noah Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Noah Smith",
    "email": {
      "value": "casey.3872@testmail.io"
    },
    "id": "cust_62ecb57ded9542ecb0c9dd6c0ad2fc81",
    "phone_number": "+913313450691"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "4990 Main St"
      },
      "line2": {
        "value": "8550 Main Dr"
      },
      "line3": {
        "value": "2066 Market Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "54341"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.4279@testmail.io"
      },
      "phone_number": {
        "value": "3222614944"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "1414 Market Ave"
      },
      "line2": {
        "value": "1795 Sunset Ave"
      },
      "line3": {
        "value": "8943 Lake Ln"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "86556"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.5511@example.com"
      },
      "phone_number": {
        "value": "4268120121"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "return_url": "https://example.com/payment/return",
  "webhook_url": "https://example.com/payment/webhook",
  "complete_authorize_url": "https://example.com/payment/complete",
  "order_category": "physical",
  "setup_future_usage": "ON_SESSION",
  "off_session": false,
  "description": "No3DS manual capture card payment (credit)",
  "payment_channel": "ECOMMERCE",
  "test_mode": true
}
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```json
{
  "status": "AUTHENTICATION_PENDING",
  "status_code": 200,
  "response_headers": {
    "content-type": "application/json",
    "x-dns-prefetch-control": "off",
    "cross-origin-resource-policy": "same-origin",
    "x-download-options": "noopen",
    "x-permitted-cross-domain-policies": "none",
    "connection": "Upgrade",
    "access-control-allow-origin": "*",
    "content-security-policy": "default-src 'self'; base-uri 'self'; font-src 'self' https: data:; form-action 'self'; frame-ancestors 'self'; img-src 'self' data:; object-src 'none'; script-src 'self'; script-src-attr 'none'; style-src 'self' https: 'unsafe-inline'; upgrade-insecure-requests",
    "x-content-type-options": "nosniff",
    "server": "Apache",
    "x-xss-protection": "0",
    "cross-origin-opener-policy": "same-origin",
    "referrer-policy": "no-referrer",
    "cross-origin-embedder-policy": "require-corp",
    "strict-transport-security": "max-age=15552000; includeSubDomains",
    "origin-agent-cluster": "?1",
    "date": "Mon, 23 Mar 2026 12:18:52 GMT",
    "content-length": "287",
    "permissions-policy": "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()",
    "upgrade": "h2,h2c",
    "x-frame-options": "SAMEORIGIN"
  },
  "redirection_data": {
    "form_type": {
      "Form": {
        "endpoint": "https://payport.novalnet.de/pci_payport/txn_secret/169653ac264e3afd3b15db17035b0080",
        "method": "HTTP_METHOD_GET",
        "form_fields": {}
      }
    }
  },
  "raw_connector_response": "***MASKED***"
}
```

</details>

</details>
<details>
<summary>Show Request (masked)</summary>

```json
{
  "connector_transaction_id": "auto_generate",
  "merchant_void_id": "mvi_336beee43dde4fbab2d86967bcb9b52f",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_order_id": "gen_122056",
  "cancellation_reason": "requested_by_customer"
}
```

</details>

<details>
<summary>Show Response (masked)</summary>

```json
{
  "status": "FAILURE",
  "error": {
    "connector_details": {
      "code": "FAILURE",
      "message": "Authentication Failed",
      "reason": "Authentication Failed"
    }
  },
  "status_code": 200,
  "response_headers": {
    "content-type": "application/json",
    "upgrade": "h2,h2c",
    "access-control-allow-origin": "*",
    "server": "Apache",
    "connection": "Upgrade",
    "date": "Mon, 23 Mar 2026 12:18:53 GMT",
    "content-length": "123"
  }
}
```

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
