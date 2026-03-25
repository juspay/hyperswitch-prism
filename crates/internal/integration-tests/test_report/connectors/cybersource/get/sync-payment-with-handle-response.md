# Connector `cybersource` / Suite `get` / Scenario `sync_payment_with_handle_response`

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
  "merchant_transaction_id": "mti_2d85a603eb3b4257872a8a0b038ffe63",
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
        "value": "Mia Johnson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Miller",
    "email": {
      "value": "casey.7825@example.com"
    },
    "id": "cust_6e96a75da7f348dab0f572f0e6766de0",
    "phone_number": "+15012698097"
  },
  "metadata": {
    "value": "{}"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "245 Lake Dr"
      },
      "line2": {
        "value": "587 Sunset St"
      },
      "line3": {
        "value": "4375 Main Ln"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "91711"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.7797@example.com"
      },
      "phone_number": {
        "value": "9027119947"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "3924 Pine Ave"
      },
      "line2": {
        "value": "9013 Sunset St"
      },
      "line3": {
        "value": "5755 Market Ln"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "28842"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.3713@example.com"
      },
      "phone_number": {
        "value": "3874778222"
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
  "test_mode": true,
  "locale": "en-US",
  "connector_feature_data": {
    "value": "{\"disable_avs\":false,\"disable_cvn\":false}"
  }
}
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```json
{
  "merchant_transaction_id": "mti_2d85a603eb3b4257872a8a0b038ffe63",
  "connector_transaction_id": "7742679114296779604805",
  "status": "CHARGED",
  "status_code": 201,
  "response_headers": {
    "content-length": "1745",
    "pragma": "no-cache",
    "x-response-time": "308ms",
    "cache-control": "no-cache, no-store, must-revalidate",
    "v-c-correlation-id": "abaafb87-3434-4dd2-9322-37fdbf41f720",
    "expires": "-1",
    "x-opnet-transaction-trace": "c6da0384-aff8-4d30-af35-f1073880e050-2349521-20098901",
    "content-type": "application/hal+json",
    "x-requestid": "7742679114296779604805",
    "strict-transport-security": "max-age=31536000"
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
  "connector_transaction_id": "7742679114296779604805",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "connector_feature_data": {
    "value": "{\"disable_avs\":false,\"disable_cvn\":false}"
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
    "v-c-correlation-id": "0fce386d-d491-43d3-a8ca-359ce718bfac",
    "last-modified": "Thu, 01 Jan 1970 00:00:00 GMT",
    "content-type": "application/hal+json;charset=utf-8",
    "x-opnet-transaction-trace": "a2_fe0adecb-bfed-4f26-b196-aadadd2a64b3-30663-7851732",
    "x-xss-protection": "1",
    "connection": "keep-alive",
    "content-length": "223",
    "cache-control": "no-cache, no-store, max-age=0",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "pragma": "no-cache",
    "etag": "\"-1955592513\"",
    "x-content-type-options": "nosniff",
    "vary": "Accept-Encoding",
    "date": "Mon, 23 Mar 2026 12:11:52 GMT",
    "content-security-policy": "default-src 'none'; script-src 'self'; connect-src 'self'; img-src 'self'; style-src 'self';",
    "expires": "-1"
  },
  "raw_connector_response": "***MASKED***"
}
```

</details>


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
