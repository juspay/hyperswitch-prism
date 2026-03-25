# Connector `novalnet` / Suite `void` / Scenario `void_authorized_payment`

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
  "merchant_transaction_id": "mti_79a4e63a2e04492f90eb7b0dfc7efdd3",
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
        "value": "Liam Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Emma Taylor",
    "email": {
      "value": "jordan.8825@example.com"
    },
    "id": "cust_becedf1439eb4226b63dbd346e712111",
    "phone_number": "+15232356205"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "164 Main Blvd"
      },
      "line2": {
        "value": "3294 Oak Dr"
      },
      "line3": {
        "value": "4537 Main Rd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "82668"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.4081@testmail.io"
      },
      "phone_number": {
        "value": "9192133314"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "9154 Pine St"
      },
      "line2": {
        "value": "6138 Market Dr"
      },
      "line3": {
        "value": "6462 Main Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "53817"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.6580@sandbox.example.com"
      },
      "phone_number": {
        "value": "1032222096"
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
    "x-permitted-cross-domain-policies": "none",
    "access-control-allow-origin": "*",
    "x-content-type-options": "nosniff",
    "cross-origin-embedder-policy": "require-corp",
    "strict-transport-security": "max-age=15552000; includeSubDomains",
    "content-security-policy": "default-src 'self'; base-uri 'self'; font-src 'self' https: data:; form-action 'self'; frame-ancestors 'self'; img-src 'self' data:; object-src 'none'; script-src 'self'; script-src-attr 'none'; style-src 'self' https: 'unsafe-inline'; upgrade-insecure-requests",
    "origin-agent-cluster": "?1",
    "x-download-options": "noopen",
    "cross-origin-opener-policy": "same-origin",
    "connection": "Upgrade",
    "x-dns-prefetch-control": "off",
    "permissions-policy": "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()",
    "cross-origin-resource-policy": "same-origin",
    "content-length": "287",
    "x-xss-protection": "0",
    "server": "Apache",
    "content-type": "application/json",
    "x-frame-options": "SAMEORIGIN",
    "referrer-policy": "no-referrer",
    "upgrade": "h2,h2c",
    "date": "Mon, 23 Mar 2026 12:18:51 GMT"
  },
  "redirection_data": {
    "form_type": {
      "Form": {
        "endpoint": "https://payport.novalnet.de/pci_payport/txn_secret/4a39772e75c84eaf16d2218daeb41a6c",
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
  "merchant_void_id": "mvi_b838a7d7160f4cbbaf825c941bbdab9a",
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
    "content-length": "123",
    "upgrade": "h2,h2c",
    "access-control-allow-origin": "*",
    "date": "Mon, 23 Mar 2026 12:18:52 GMT",
    "content-type": "application/json",
    "connection": "Upgrade",
    "server": "Apache"
  }
}
```

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
