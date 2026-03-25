# Connector `novalnet` / Suite `refund_sync` / Scenario `refund_sync`

- Service: `RefundService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
unsupported suite 'refund_sync' for grpcurl generation
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
  "merchant_transaction_id": "mti_73156df062bf4935b8543a06aa3f17d1",
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
        "value": "Noah Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Emma Taylor",
    "email": {
      "value": "sam.8860@sandbox.example.com"
    },
    "id": "cust_7427652261f94408b03b9091bfcccefd",
    "phone_number": "+919211226263"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "6796 Pine Blvd"
      },
      "line2": {
        "value": "7229 Pine Dr"
      },
      "line3": {
        "value": "5397 Lake Rd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "91204"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.3489@sandbox.example.com"
      },
      "phone_number": {
        "value": "9124960551"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "1770 Market Rd"
      },
      "line2": {
        "value": "2351 Main St"
      },
      "line3": {
        "value": "2632 Market St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "33101"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.7798@example.com"
      },
      "phone_number": {
        "value": "1063428258"
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
    "date": "Mon, 23 Mar 2026 12:18:47 GMT",
    "cross-origin-resource-policy": "same-origin",
    "x-permitted-cross-domain-policies": "none",
    "referrer-policy": "no-referrer",
    "content-type": "application/json",
    "x-frame-options": "SAMEORIGIN",
    "x-xss-protection": "0",
    "upgrade": "h2,h2c",
    "content-length": "287",
    "cross-origin-opener-policy": "same-origin",
    "x-dns-prefetch-control": "off",
    "server": "Apache",
    "strict-transport-security": "max-age=15552000; includeSubDomains",
    "content-security-policy": "default-src 'self'; base-uri 'self'; font-src 'self' https: data:; form-action 'self'; frame-ancestors 'self'; img-src 'self' data:; object-src 'none'; script-src 'self'; script-src-attr 'none'; style-src 'self' https: 'unsafe-inline'; upgrade-insecure-requests",
    "permissions-policy": "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()",
    "origin-agent-cluster": "?1",
    "x-content-type-options": "nosniff",
    "cross-origin-embedder-policy": "require-corp",
    "connection": "Upgrade",
    "x-download-options": "noopen"
  },
  "redirection_data": {
    "form_type": {
      "Form": {
        "endpoint": "https://payport.novalnet.de/pci_payport/txn_secret/3e06c6d4791c307faca40b0f63bdb603",
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


[Back to Connector Suite](../refund-sync.md) | [Back to Overview](../../../test_overview.md)
