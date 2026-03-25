# Connector `cybersource` / Suite `get` / Scenario `sync_payment`

- Service: `PaymentService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'error': expected field to be absent or null, got {"issuer_details":{"network_details":{}},"connector_details":{"code":"No error code","message":"The requested resource does not exist","reason":"The requested resource does not exist"}}
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
  "merchant_transaction_id": "mti_0ec2258fd43942e6ae62b83728909632",
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
        "value": "Mia Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ava Smith",
    "email": {
      "value": "riley.7722@sandbox.example.com"
    },
    "id": "cust_f7320fecb4d6452285f8d4da48c83338",
    "phone_number": "+912625174105"
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
        "value": "534 Lake Blvd"
      },
      "line2": {
        "value": "4568 Market Ave"
      },
      "line3": {
        "value": "5857 Oak Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "98913"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.2927@testmail.io"
      },
      "phone_number": {
        "value": "1965897282"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "4926 Oak Blvd"
      },
      "line2": {
        "value": "1774 Oak St"
      },
      "line3": {
        "value": "3694 Oak St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "56981"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.9138@example.com"
      },
      "phone_number": {
        "value": "7310761140"
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
  "merchant_transaction_id": "mti_0ec2258fd43942e6ae62b83728909632",
  "connector_transaction_id": "7742679093156778004805",
  "status": "CHARGED",
  "status_code": 201,
  "response_headers": {
    "cache-control": "no-cache, no-store, must-revalidate",
    "content-length": "1745",
    "x-requestid": "7742679093156778004805",
    "x-response-time": "256ms",
    "strict-transport-security": "max-age=31536000",
    "x-opnet-transaction-trace": "c6da0384-aff8-4d30-af35-f1073880e050-2349521-20098532",
    "expires": "-1",
    "pragma": "no-cache",
    "v-c-correlation-id": "fa6460fa-5fc7-466b-879b-e802b71d2a15",
    "content-type": "application/hal+json"
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
  "connector_transaction_id": "7742679093156778004805",
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
    "x-xss-protection": "1",
    "connection": "keep-alive",
    "content-security-policy": "default-src 'none'; script-src 'self'; connect-src 'self'; img-src 'self'; style-src 'self';",
    "cache-control": "no-cache, no-store, max-age=0",
    "etag": "\"1058504087\"",
    "v-c-correlation-id": "95f88106-72df-4fe7-88c3-ee3d0550d498",
    "x-opnet-transaction-trace": "a2_fe0adecb-bfed-4f26-b196-aadadd2a64b3-30663-7851690",
    "pragma": "no-cache",
    "content-type": "application/hal+json;charset=utf-8",
    "expires": "-1",
    "vary": "Accept-Encoding",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "date": "Mon, 23 Mar 2026 12:11:50 GMT",
    "content-length": "223",
    "last-modified": "Thu, 01 Jan 1970 00:00:00 GMT",
    "x-content-type-options": "nosniff"
  },
  "raw_connector_response": "***MASKED***"
}
```

</details>


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
