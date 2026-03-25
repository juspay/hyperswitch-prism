# Connector `novalnet` / Suite `void` / Scenario `void_without_cancellation_reason`

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
  "merchant_transaction_id": "mti_24c1368cfa7e45aca2b2b49fa93ef6d3",
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
        "value": "Emma Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Noah Taylor",
    "email": {
      "value": "morgan.9956@sandbox.example.com"
    },
    "id": "cust_4d8ab344d7634ca48cf8c02de6942691",
    "phone_number": "+445018999272"
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
        "value": "6479 Oak Ln"
      },
      "line2": {
        "value": "7541 Main St"
      },
      "line3": {
        "value": "3935 Main Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "66199"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.9650@sandbox.example.com"
      },
      "phone_number": {
        "value": "8572972638"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "9857 Oak Dr"
      },
      "line2": {
        "value": "8777 Lake Ln"
      },
      "line3": {
        "value": "9733 Oak Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "67173"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.5501@testmail.io"
      },
      "phone_number": {
        "value": "3031490669"
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
    "access-control-allow-origin": "*",
    "content-length": "287",
    "permissions-policy": "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()",
    "x-frame-options": "SAMEORIGIN",
    "date": "Mon, 23 Mar 2026 12:18:54 GMT",
    "x-dns-prefetch-control": "off",
    "upgrade": "h2,h2c",
    "cross-origin-embedder-policy": "require-corp",
    "content-type": "application/json",
    "strict-transport-security": "max-age=15552000; includeSubDomains",
    "x-xss-protection": "0",
    "x-content-type-options": "nosniff",
    "content-security-policy": "default-src 'self'; base-uri 'self'; font-src 'self' https: data:; form-action 'self'; frame-ancestors 'self'; img-src 'self' data:; object-src 'none'; script-src 'self'; script-src-attr 'none'; style-src 'self' https: 'unsafe-inline'; upgrade-insecure-requests",
    "connection": "Upgrade",
    "origin-agent-cluster": "?1",
    "referrer-policy": "no-referrer",
    "cross-origin-opener-policy": "same-origin",
    "cross-origin-resource-policy": "same-origin",
    "server": "Apache",
    "x-permitted-cross-domain-policies": "none",
    "x-download-options": "noopen"
  },
  "redirection_data": {
    "form_type": {
      "Form": {
        "endpoint": "https://payport.novalnet.de/pci_payport/txn_secret/dfeccbe8d5769acdd95dd92de7fb4383",
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
  "merchant_void_id": "mvi_c18b6abac5d24c40ad297c8b96fe83d3"
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
    "connection": "Upgrade",
    "access-control-allow-origin": "*",
    "upgrade": "h2,h2c",
    "server": "Apache",
    "date": "Mon, 23 Mar 2026 12:18:55 GMT",
    "content-length": "123"
  }
}
```

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
