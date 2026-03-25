# Connector `fiserv` / Suite `void` / Scenario `void_without_cancellation_reason`

- Service: `PaymentService/Void`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
sdk call failed: sdk request transformer failed for 'void/void_without_cancellation_reason': Failed to encode connector request (code: INTERNAL_SERVER_ERROR)
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
  "merchant_transaction_id": "mti_deda23a07488464791425eca48311252",
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
        "value": "Noah Taylor"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ethan Miller",
    "email": {
      "value": "alex.5662@sandbox.example.com"
    },
    "id": "cust_c1d61c9d64ab4c37b0a9f857acf9e5a4",
    "phone_number": "+16127192688"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "8298 Pine Blvd"
      },
      "line2": {
        "value": "5372 Lake Blvd"
      },
      "line3": {
        "value": "5496 Sunset Ave"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "39800"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.1529@testmail.io"
      },
      "phone_number": {
        "value": "9984645016"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "706 Oak Ave"
      },
      "line2": {
        "value": "6003 Pine Ave"
      },
      "line3": {
        "value": "1427 Pine Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "49469"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.5690@example.com"
      },
      "phone_number": {
        "value": "2007491531"
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
    "x-frame-options": "DENY",
    "expires": "0",
    "referrer-policy": "no-referrer",
    "pragma": "no-cache",
    "x-content-type-options": "nosniff",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-request-id": "04552fc3-9e05-484b-991b-e0cb179d32953747.1",
    "content-type": "application/json",
    "set-cookie": "__uzmd=1774268007; HttpOnly; path=/; Expires=Mon, 21-Sep-26 12:13:27 GMT; Max-Age=15724800; SameSite=Lax",
    "rdwr_response": "allowed",
    "content-security-policy": "default-src 'none'; script-src 'strict-dynamic' 'nonce-827c0bf9afb569e666941ce4ba3356e9'; frame-ancestors 'none'",
    "date": "Mon, 23 Mar 2026 12:13:27 GMT",
    "apitraceid": "b6ebb43598624ec4bea3accb66856426",
    "transactionprocessedin": "chandler",
    "targetserverreceivedendtimestamp": "1774268007787",
    "access-control-allow-headers": "api-key,auth-token-type,authorization,client-request-id,content-type,message-digest,timestamp,x-integration,x-integration-merchant-id,x-integration-origin,x-integration-terminal-id,x-integration-version,x-deploymentrouteto",
    "connection": "keep-alive",
    "x-xss-protection": "1; mode=block",
    "targetserversentstarttimestamp": "1774268007728",
    "content-length": "421",
    "access-control-allow-origin": "",
    "x-vcap-request-id": "b6ebb435-9862-4ec4-bea3-accb66856426",
    "cache-control": "no-store, no-cache, must-revalidate",
    "access-control-max-age": "86400"
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
