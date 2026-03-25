# Connector `nexinets` / Suite `void` / Scenario `void_without_cancellation_reason`

- Service: `PaymentService/Void`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
sdk call failed: sdk request transformer failed for 'void/void_without_cancellation_reason': Missing required field: connector_meta_data (code: BAD_REQUEST)
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
  "merchant_transaction_id": "mti_9d4dfec147334b4e9f88c6743ad266b9",
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
        "value": "Noah Johnson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Johnson",
    "email": {
      "value": "morgan.3301@sandbox.example.com"
    },
    "id": "cust_d50b917ca4ce4470a8a8c5ed89f3f123",
    "phone_number": "+447243384607"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "4682 Market Blvd"
      },
      "line2": {
        "value": "9296 Sunset St"
      },
      "line3": {
        "value": "5683 Lake Rd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "18789"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.3085@testmail.io"
      },
      "phone_number": {
        "value": "8896632357"
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
        "value": "4084 Market Rd"
      },
      "line2": {
        "value": "9300 Lake Blvd"
      },
      "line3": {
        "value": "2500 Sunset Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "48389"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.3959@sandbox.example.com"
      },
      "phone_number": {
        "value": "6559345633"
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
  "status": "ATTEMPT_STATUS_UNSPECIFIED",
  "error": {
    "issuer_details": {
      "network_details": {}
    },
    "connector_details": {
      "code": "12000",
      "message": "Bad value for 'merchantOrderId'. Expected: unique string between 1 and 30 characters.",
      "reason": "reason : Error while creating order. , message : Bad value for 'merchantOrderId'. Expected: unique string between 1 and 30 characters."
    }
  },
  "status_code": 400,
  "response_headers": {
    "x-evasion-track-id": "acEvE2itqJA9RElhojtgoAAAAAA",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "cache-control": "no-store",
    "content-type": "application/json",
    "access-control-allow-credentials": "true",
    "x-content-type-options": "nosniff",
    "connection": "close",
    "access-control-allow-methods": "GET, POST, PUT, PATCH, DELETE, OPTIONS, HEAD",
    "x-xss-protection": "1; mode=block",
    "date": "Mon, 23 Mar 2026 12:16:19 GMT",
    "access-control-allow-headers": "origin, content-type, accept, authorization",
    "content-security-policy": "default-src 'none'; script-src 'self' *.payengine.de www.google-analytics.com 'unsafe-inline'; connect-src 'self' *.payengine.de; img-src 'self' www.google-analytics.com data:; style-src 'self' 'unsafe-inline'; font-src 'self';",
    "access-control-allow-origin": "*",
    "pragma": "no-cache",
    "content-length": "301",
    "server": "Apache"
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


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
