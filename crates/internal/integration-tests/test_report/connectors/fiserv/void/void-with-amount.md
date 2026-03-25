# Connector `fiserv` / Suite `void` / Scenario `void_with_amount`

- Service: `PaymentService/Void`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
sdk call failed: sdk request transformer failed for 'void/void_with_amount': Failed to encode connector request (code: INTERNAL_SERVER_ERROR)
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
  "merchant_transaction_id": "mti_f4b013d9aacf40f2904825901e6a1196",
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
  "capture_method": "MANUAL",
  "customer": {
    "name": "Mia Smith",
    "email": {
      "value": "riley.7165@example.com"
    },
    "id": "cust_8794a9adec344091b41cc0be1019de4d",
    "phone_number": "+19082100307"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "4169 Lake Rd"
      },
      "line2": {
        "value": "4551 Market Blvd"
      },
      "line3": {
        "value": "4031 Pine Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "97799"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.9088@sandbox.example.com"
      },
      "phone_number": {
        "value": "7734704906"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "2842 Pine Rd"
      },
      "line2": {
        "value": "7076 Pine Dr"
      },
      "line3": {
        "value": "7676 Oak Ln"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "89724"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.8342@testmail.io"
      },
      "phone_number": {
        "value": "7777568421"
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
      "code": "202",
      "message": "Invalid Terminal ID or Setup"
    }
  },
  "status_code": 400,
  "response_headers": {
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "date": "Mon, 23 Mar 2026 12:13:26 GMT",
    "apitraceid": "f8436719f4684bfbb45a16a071b65368",
    "content-type": "application/json",
    "x-content-type-options": "nosniff",
    "x-request-id": "04552fc3-9e05-484b-991b-e0cb179d32953745.1",
    "x-xss-protection": "1; mode=block",
    "referrer-policy": "no-referrer",
    "cache-control": "no-store, no-cache, must-revalidate",
    "content-security-policy": "default-src 'none'; script-src 'strict-dynamic' 'nonce-cb42f0500006922e9e6f05a8a150458c'; frame-ancestors 'none'",
    "x-frame-options": "DENY",
    "connection": "keep-alive",
    "targetserversentstarttimestamp": "1774268006494",
    "set-cookie": "__uzmd=1774268006; HttpOnly; path=/; Expires=Mon, 21-Sep-26 12:13:26 GMT; Max-Age=15724800; SameSite=Lax",
    "x-vcap-request-id": "f8436719-f468-4bfb-b45a-16a071b65368",
    "rdwr_response": "allowed",
    "access-control-allow-origin": "",
    "expires": "0",
    "access-control-max-age": "86400",
    "pragma": "no-cache",
    "targetserverreceivedendtimestamp": "1774268006559",
    "content-length": "421",
    "access-control-allow-headers": "api-key,auth-token-type,authorization,client-request-id,content-type,message-digest,timestamp,x-integration,x-integration-merchant-id,x-integration-origin,x-integration-terminal-id,x-integration-version,x-deploymentrouteto",
    "transactionprocessedin": "chandler"
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
