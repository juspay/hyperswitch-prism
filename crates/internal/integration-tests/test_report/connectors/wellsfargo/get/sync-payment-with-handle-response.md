# Connector `wellsfargo` / Suite `get` / Scenario `sync_payment_with_handle_response`

- Service: `PaymentService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
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
<summary>2. authorize(no3ds_auto_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```json
{
  "merchant_transaction_id": "mti_76dcf2640ea14868b0a1aa0b1a3f4f35",
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
        "value": "Ethan Brown"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Johnson",
    "email": {
      "value": "sam.3613@sandbox.example.com"
    },
    "id": "cust_676d67b260d34d5c95a2c6f8887fa7ff",
    "phone_number": "+918435957330"
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
        "value": "3255 Main Ln"
      },
      "line2": {
        "value": "8665 Lake St"
      },
      "line3": {
        "value": "2563 Lake St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "50378"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.1669@testmail.io"
      },
      "phone_number": {
        "value": "1354391649"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "3209 Pine Ave"
      },
      "line2": {
        "value": "5997 Main Ln"
      },
      "line3": {
        "value": "2348 Oak St"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "74131"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.5878@sandbox.example.com"
      },
      "phone_number": {
        "value": "4146301612"
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
  "merchant_transaction_id": "mti_76dcf2640ea14868b0a1aa0b1a3f4f35",
  "connector_transaction_id": "7742691053086577804806",
  "status": "CHARGED",
  "status_code": 201,
  "response_headers": {
    "cache-control": "no-cache, no-store, must-revalidate",
    "expires": "-1",
    "x-opnet-transaction-trace": "687b1be9-a770-4c1b-984d-e09c8a4b9f43-2283790-19812805",
    "content-type": "application/hal+json",
    "strict-transport-security": "max-age=31536000",
    "x-response-time": "86ms",
    "content-length": "1125",
    "x-requestid": "7742691053086577804806",
    "pragma": "no-cache",
    "v-c-correlation-id": "acafbc51-e2f7-4467-afb3-71967c2bc965"
  },
  "network_transaction_id": "016150703802094",
  "incremental_authorization_allowed": "***MASKED***",
  "raw_connector_response": "***MASKED***",
  "connector_response": {
    "additional_payment_method_data": {
      "payment_method_data": {
        "Card": {
          "payment_checks": [
            123,
            34,
            97,
            118,
            115,
            95,
            114,
            101,
            115,
            112,
            111,
            110,
            115,
            101,
            34,
            58,
            123,
            34,
            99,
            111,
            100,
            101,
            34,
            58,
            34,
            89,
            34,
            44,
            34,
            99,
            111,
            100,
            101,
            82,
            97,
            119,
            34,
            58,
            34,
            89,
            34,
            125,
            44,
            34,
            99,
            97,
            114,
            100,
            95,
            118,
            101,
            114,
            105,
            102,
            105,
            99,
            97,
            116,
            105,
            111,
            110,
            34,
            58,
            110,
            117,
            108,
            108,
            125
          ]
        }
      }
    }
  }
}
```

</details>

</details>
<details>
<summary>Show Request (masked)</summary>

```json
{
  "connector_transaction_id": "7742691053086577804806",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  }
}
```

</details>

<details>
<summary>Show Response (masked)</summary>

```json
{
  "status": "ATTEMPT_STATUS_UNSPECIFIED",
  "error": {
    "issuer_details": {
      "network_details": {}
    },
    "connector_details": {
      "code": "No error code",
      "message": "The requested resource does not exist",
      "reason": "The requested resource does not exist"
    }
  },
  "status_code": 404,
  "response_headers": {
    "expires": "-1",
    "content-length": "211",
    "pragma": "no-cache",
    "x-response-time": "26ms",
    "strict-transport-security": "max-age=31536000",
    "x-opnet-transaction-trace": "c6da0384-aff8-4d30-af35-f1073880e050-2349521-20332335",
    "v-c-correlation-id": "54e23e06-5bf8-46f5-a39f-3ff42682c0b3",
    "x-requestid": "7742691063006260204805",
    "cache-control": "no-cache, no-store, must-revalidate",
    "content-type": "application/hal+json",
    "connection": "keep-alive"
  },
  "raw_connector_response": "***MASKED***"
}
```

</details>


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
