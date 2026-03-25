# Connector `novalnet` / Suite `get` / Scenario `sync_payment_with_handle_response`

- Service: `PaymentService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
sdk call failed: sdk response transformer failed for 'get/sync_payment_with_handle_response': Failed to deserialize connector response (code: INTERNAL_SERVER_ERROR)
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
  "merchant_transaction_id": "mti_26f89c05c2c746bfa672cac9519158ee",
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
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Johnson",
    "email": {
      "value": "casey.7195@sandbox.example.com"
    },
    "id": "cust_ba654a65f1cd4684a36fa8660a4493a2",
    "phone_number": "+449853632369"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "9065 Pine Rd"
      },
      "line2": {
        "value": "8314 Market Blvd"
      },
      "line3": {
        "value": "2646 Main Rd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "81122"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.6347@example.com"
      },
      "phone_number": {
        "value": "6461745824"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "3663 Market Dr"
      },
      "line2": {
        "value": "9291 Oak Dr"
      },
      "line3": {
        "value": "6935 Oak Rd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "54892"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.7924@testmail.io"
      },
      "phone_number": {
        "value": "9625073062"
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
    "access-control-allow-origin": "*",
    "x-frame-options": "SAMEORIGIN",
    "content-security-policy": "default-src 'self'; base-uri 'self'; font-src 'self' https: data:; form-action 'self'; frame-ancestors 'self'; img-src 'self' data:; object-src 'none'; script-src 'self'; script-src-attr 'none'; style-src 'self' https: 'unsafe-inline'; upgrade-insecure-requests",
    "x-xss-protection": "0",
    "content-length": "287",
    "x-permitted-cross-domain-policies": "none",
    "permissions-policy": "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()",
    "upgrade": "h2,h2c",
    "cross-origin-opener-policy": "same-origin",
    "content-type": "application/json",
    "cross-origin-resource-policy": "same-origin",
    "strict-transport-security": "max-age=15552000; includeSubDomains",
    "referrer-policy": "no-referrer",
    "connection": "Upgrade",
    "x-download-options": "noopen",
    "x-dns-prefetch-control": "off",
    "server": "Apache",
    "x-content-type-options": "nosniff",
    "date": "Mon, 23 Mar 2026 12:18:46 GMT",
    "cross-origin-embedder-policy": "require-corp",
    "origin-agent-cluster": "?1"
  },
  "redirection_data": {
    "form_type": {
      "Form": {
        "endpoint": "https://payport.novalnet.de/pci_payport/txn_secret/c234bab785998e89ca4eec0c4faba82c",
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
