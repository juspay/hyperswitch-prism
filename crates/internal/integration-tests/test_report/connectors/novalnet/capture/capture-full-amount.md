# Connector `novalnet` / Suite `capture` / Scenario `capture_full_amount`

- Service: `PaymentService/Capture`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
sdk call failed: sdk response transformer failed for 'capture/capture_full_amount': Failed to deserialize connector response (code: INTERNAL_SERVER_ERROR)
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
  "merchant_transaction_id": "mti_f46dbb647111472f87278ab52318e334",
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
    "name": "Liam Johnson",
    "email": {
      "value": "casey.7233@testmail.io"
    },
    "id": "cust_f74847199f364de68d4ea25ed058c0d9",
    "phone_number": "+17479405820"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "2280 Pine Blvd"
      },
      "line2": {
        "value": "1921 Lake Blvd"
      },
      "line3": {
        "value": "2042 Pine Rd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "52356"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.9070@sandbox.example.com"
      },
      "phone_number": {
        "value": "4674960191"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "4233 Lake Blvd"
      },
      "line2": {
        "value": "9160 Oak St"
      },
      "line3": {
        "value": "3516 Main Ln"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "39548"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.2241@testmail.io"
      },
      "phone_number": {
        "value": "9362325719"
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
    "upgrade": "h2,h2c",
    "x-frame-options": "SAMEORIGIN",
    "x-permitted-cross-domain-policies": "none",
    "permissions-policy": "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()",
    "content-length": "287",
    "strict-transport-security": "max-age=15552000; includeSubDomains",
    "x-download-options": "noopen",
    "content-type": "application/json",
    "x-dns-prefetch-control": "off",
    "cross-origin-embedder-policy": "require-corp",
    "content-security-policy": "default-src 'self'; base-uri 'self'; font-src 'self' https: data:; form-action 'self'; frame-ancestors 'self'; img-src 'self' data:; object-src 'none'; script-src 'self'; script-src-attr 'none'; style-src 'self' https: 'unsafe-inline'; upgrade-insecure-requests",
    "server": "Apache",
    "cross-origin-opener-policy": "same-origin",
    "referrer-policy": "no-referrer",
    "connection": "Upgrade",
    "origin-agent-cluster": "?1",
    "date": "Mon, 23 Mar 2026 12:18:39 GMT",
    "access-control-allow-origin": "*",
    "x-xss-protection": "0",
    "cross-origin-resource-policy": "same-origin",
    "x-content-type-options": "nosniff"
  },
  "redirection_data": {
    "form_type": {
      "Form": {
        "endpoint": "https://payport.novalnet.de/pci_payport/txn_secret/7619acde19570686e831f07ab753677d",
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
