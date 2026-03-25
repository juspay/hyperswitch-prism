# Connector `novalnet` / Suite `capture` / Scenario `capture_partial_amount`

- Service: `PaymentService/Capture`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
sdk call failed: sdk response transformer failed for 'capture/capture_partial_amount': Failed to deserialize connector response (code: INTERNAL_SERVER_ERROR)
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
  "merchant_transaction_id": "mti_af6e5c17adc24b1c9f0efab6dea8ef26",
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
        "value": "Emma Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Taylor",
    "email": {
      "value": "sam.7078@example.com"
    },
    "id": "cust_96ea50c47d874de2997e8e3fd146a3d0",
    "phone_number": "+916195236025"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "9226 Oak St"
      },
      "line2": {
        "value": "5134 Sunset Rd"
      },
      "line3": {
        "value": "2403 Oak Ave"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "14294"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.4214@example.com"
      },
      "phone_number": {
        "value": "7881037093"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "8374 Sunset Ave"
      },
      "line2": {
        "value": "3510 Main Dr"
      },
      "line3": {
        "value": "3376 Pine Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "67660"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.4638@testmail.io"
      },
      "phone_number": {
        "value": "2276320688"
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
    "content-length": "287",
    "x-download-options": "noopen",
    "cross-origin-embedder-policy": "require-corp",
    "x-frame-options": "SAMEORIGIN",
    "content-type": "application/json",
    "upgrade": "h2,h2c",
    "x-xss-protection": "0",
    "strict-transport-security": "max-age=15552000; includeSubDomains",
    "connection": "Upgrade",
    "content-security-policy": "default-src 'self'; base-uri 'self'; font-src 'self' https: data:; form-action 'self'; frame-ancestors 'self'; img-src 'self' data:; object-src 'none'; script-src 'self'; script-src-attr 'none'; style-src 'self' https: 'unsafe-inline'; upgrade-insecure-requests",
    "x-dns-prefetch-control": "off",
    "permissions-policy": "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()",
    "referrer-policy": "no-referrer",
    "server": "Apache",
    "cross-origin-opener-policy": "same-origin",
    "origin-agent-cluster": "?1",
    "access-control-allow-origin": "*",
    "x-permitted-cross-domain-policies": "none",
    "x-content-type-options": "nosniff",
    "date": "Mon, 23 Mar 2026 12:18:41 GMT",
    "cross-origin-resource-policy": "same-origin"
  },
  "redirection_data": {
    "form_type": {
      "Form": {
        "endpoint": "https://payport.novalnet.de/pci_payport/txn_secret/47ae75d900d48c1ac799661fd6e32fa8",
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

_Request trace not available._

</details>

<details>
<summary>Show Response (masked)</summary>

_Response trace not available._

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
