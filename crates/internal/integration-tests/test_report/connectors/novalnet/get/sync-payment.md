# Connector `novalnet` / Suite `get` / Scenario `sync_payment`

- Service: `PaymentService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
sdk call failed: sdk response transformer failed for 'get/sync_payment': Failed to deserialize connector response (code: INTERNAL_SERVER_ERROR)
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
<summary>2. authorize(no3ds_auto_capture_credit_card) — FAIL</summary>

**Dependency Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

<details>
<summary>Show Dependency Request (masked)</summary>

```json
{
  "merchant_transaction_id": "mti_3c870ab16ca84162a88f10c2748f53a7",
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
        "value": "Mia Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Brown",
    "email": {
      "value": "morgan.3916@testmail.io"
    },
    "id": "cust_2a21defa534a4bc486dc19e95f3f50ae",
    "phone_number": "+12136652135"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "2951 Market Ave"
      },
      "line2": {
        "value": "6272 Market Ave"
      },
      "line3": {
        "value": "5074 Sunset Ln"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "14980"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.1183@example.com"
      },
      "phone_number": {
        "value": "2949438189"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "2508 Lake Ln"
      },
      "line2": {
        "value": "7121 Pine Dr"
      },
      "line3": {
        "value": "4416 Lake Ln"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "52843"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.4999@example.com"
      },
      "phone_number": {
        "value": "9337767646"
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
  "description": "No3DS auto capture card payment (credit)",
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
    "content-length": "287",
    "cross-origin-opener-policy": "same-origin",
    "content-type": "application/json",
    "access-control-allow-origin": "*",
    "connection": "Upgrade",
    "x-content-type-options": "nosniff",
    "cross-origin-embedder-policy": "require-corp",
    "x-download-options": "noopen",
    "cross-origin-resource-policy": "same-origin",
    "origin-agent-cluster": "?1",
    "strict-transport-security": "max-age=15552000; includeSubDomains",
    "x-permitted-cross-domain-policies": "none",
    "x-dns-prefetch-control": "off",
    "date": "Mon, 23 Mar 2026 12:18:44 GMT",
    "server": "Apache",
    "content-security-policy": "default-src 'self'; base-uri 'self'; font-src 'self' https: data:; form-action 'self'; frame-ancestors 'self'; img-src 'self' data:; object-src 'none'; script-src 'self'; script-src-attr 'none'; style-src 'self' https: 'unsafe-inline'; upgrade-insecure-requests",
    "referrer-policy": "no-referrer",
    "x-xss-protection": "0",
    "permissions-policy": "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()",
    "x-frame-options": "SAMEORIGIN"
  },
  "redirection_data": {
    "form_type": {
      "Form": {
        "endpoint": "https://payport.novalnet.de/pci_payport/txn_secret/611289f8650bef976f876b7888bb798e",
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


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
