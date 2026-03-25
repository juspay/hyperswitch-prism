# Connector `fiserv` / Suite `void` / Scenario `void_authorized_payment`

- Service: `PaymentService/Void`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
sdk call failed: sdk request transformer failed for 'void/void_authorized_payment': Failed to encode connector request (code: INTERNAL_SERVER_ERROR)
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
  "merchant_transaction_id": "mti_24b8b63b4700440a86f53abc9416deed",
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
    "name": "Emma Johnson",
    "email": {
      "value": "riley.4629@testmail.io"
    },
    "id": "cust_a4cf9c27333e40dcbb94da05265758ca",
    "phone_number": "+446581083524"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "2184 Main Ln"
      },
      "line2": {
        "value": "9310 Lake St"
      },
      "line3": {
        "value": "7492 Lake Dr"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "70219"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.2205@example.com"
      },
      "phone_number": {
        "value": "9817031412"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "6182 Pine Ave"
      },
      "line2": {
        "value": "3320 Oak Rd"
      },
      "line3": {
        "value": "5739 Oak St"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "30556"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.5779@sandbox.example.com"
      },
      "phone_number": {
        "value": "9899513833"
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
    "cache-control": "no-store, no-cache, must-revalidate",
    "pragma": "no-cache",
    "expires": "0",
    "date": "Mon, 23 Mar 2026 12:13:25 GMT",
    "targetserversentstarttimestamp": "1774268005253",
    "transactionprocessedin": "chandler",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-content-type-options": "nosniff",
    "content-length": "421",
    "access-control-allow-headers": "api-key,auth-token-type,authorization,client-request-id,content-type,message-digest,timestamp,x-integration,x-integration-merchant-id,x-integration-origin,x-integration-terminal-id,x-integration-version,x-deploymentrouteto",
    "x-request-id": "b9c38374-e1ba-4f0c-ad46-17ef94535b43871417.1",
    "targetserverreceivedendtimestamp": "1774268005321",
    "x-frame-options": "DENY",
    "content-security-policy": "default-src 'none'; script-src 'strict-dynamic' 'nonce-cb42f0500006922e9e6f05a8a150458c'; frame-ancestors 'none'",
    "x-vcap-request-id": "d88c6583-3cb9-4316-a7a3-6c8dd3cfa2ca",
    "access-control-max-age": "86400",
    "apitraceid": "d88c65833cb94316a7a36c8dd3cfa2ca",
    "rdwr_response": "allowed",
    "access-control-allow-origin": "",
    "content-type": "application/json",
    "set-cookie": "__uzmd=1774268005; HttpOnly; path=/; Expires=Mon, 21-Sep-26 12:13:25 GMT; Max-Age=15724800; SameSite=Lax",
    "referrer-policy": "no-referrer",
    "connection": "keep-alive",
    "x-xss-protection": "1; mode=block"
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
