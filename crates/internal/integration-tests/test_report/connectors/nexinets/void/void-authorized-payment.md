# Connector `nexinets` / Suite `void` / Scenario `void_authorized_payment`

- Service: `PaymentService/Void`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
sdk call failed: sdk request transformer failed for 'void/void_authorized_payment': Missing required field: connector_meta_data (code: BAD_REQUEST)
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
  "merchant_transaction_id": "mti_53d0896e995a46c381f910ed7ec9aebf",
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
        "value": "Liam Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Emma Brown",
    "email": {
      "value": "alex.3825@testmail.io"
    },
    "id": "cust_2347d906d1cd45a69e96cb89cb47de63",
    "phone_number": "+19929170241"
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
        "value": "8109 Pine St"
      },
      "line2": {
        "value": "7525 Market Blvd"
      },
      "line3": {
        "value": "9407 Main Ave"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "14926"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.9372@sandbox.example.com"
      },
      "phone_number": {
        "value": "9202578802"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "4003 Sunset Rd"
      },
      "line2": {
        "value": "9942 Lake Ln"
      },
      "line3": {
        "value": "8033 Main St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "42526"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.3765@example.com"
      },
      "phone_number": {
        "value": "5811093555"
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
    "x-evasion-track-id": "acEvEh7TiGO4Gbh2KghqrgAAAAs",
    "content-length": "301",
    "access-control-allow-origin": "*",
    "access-control-allow-methods": "GET, POST, PUT, PATCH, DELETE, OPTIONS, HEAD",
    "x-content-type-options": "nosniff",
    "date": "Mon, 23 Mar 2026 12:16:18 GMT",
    "pragma": "no-cache",
    "content-security-policy": "default-src 'none'; script-src 'self' *.payengine.de www.google-analytics.com 'unsafe-inline'; connect-src 'self' *.payengine.de; img-src 'self' www.google-analytics.com data:; style-src 'self' 'unsafe-inline'; font-src 'self';",
    "server": "Apache",
    "access-control-allow-headers": "origin, content-type, accept, authorization",
    "content-type": "application/json",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-xss-protection": "1; mode=block",
    "access-control-allow-credentials": "true",
    "cache-control": "no-store",
    "connection": "close"
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
