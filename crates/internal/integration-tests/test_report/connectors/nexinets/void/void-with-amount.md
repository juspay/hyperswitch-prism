# Connector `nexinets` / Suite `void` / Scenario `void_with_amount`

- Service: `PaymentService/Void`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
sdk call failed: sdk request transformer failed for 'void/void_with_amount': Missing required field: connector_meta_data (code: BAD_REQUEST)
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
  "merchant_transaction_id": "mti_226c63cf8774478bb3c285f01ae2bded",
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
        "value": "Liam Johnson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Mia Johnson",
    "email": {
      "value": "casey.4331@example.com"
    },
    "id": "cust_725fc005c2194568a0d984af81be3015",
    "phone_number": "+447499490864"
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
        "value": "9069 Main Ave"
      },
      "line2": {
        "value": "5139 Oak Dr"
      },
      "line3": {
        "value": "7779 Market St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "17689"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.1954@example.com"
      },
      "phone_number": {
        "value": "8976525165"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "5122 Pine Ave"
      },
      "line2": {
        "value": "2952 Oak Blvd"
      },
      "line3": {
        "value": "207 Oak St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "93320"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.5424@testmail.io"
      },
      "phone_number": {
        "value": "1998240721"
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
    "connection": "close",
    "x-xss-protection": "1; mode=block",
    "cache-control": "no-store",
    "date": "Mon, 23 Mar 2026 12:16:18 GMT",
    "content-length": "301",
    "access-control-allow-methods": "GET, POST, PUT, PATCH, DELETE, OPTIONS, HEAD",
    "access-control-allow-headers": "origin, content-type, accept, authorization",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "content-security-policy": "default-src 'none'; script-src 'self' *.payengine.de www.google-analytics.com 'unsafe-inline'; connect-src 'self' *.payengine.de; img-src 'self' www.google-analytics.com data:; style-src 'self' 'unsafe-inline'; font-src 'self';",
    "server": "Apache",
    "pragma": "no-cache",
    "access-control-allow-origin": "*",
    "access-control-allow-credentials": "true",
    "x-content-type-options": "nosniff",
    "x-evasion-track-id": "acEvEh3AoAMllYprfGrldAAAAAc",
    "content-type": "application/json"
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
