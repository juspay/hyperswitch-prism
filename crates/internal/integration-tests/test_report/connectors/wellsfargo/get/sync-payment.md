# Connector `wellsfargo` / Suite `get` / Scenario `sync_payment`

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
  "merchant_transaction_id": "mti_30a14da97aba4282b6d6f94e2382fdf2",
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
        "value": "Liam Taylor"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Taylor",
    "email": {
      "value": "sam.1005@testmail.io"
    },
    "id": "cust_e4e0202224ca4d27952bd010393123db",
    "phone_number": "+448812924096"
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
        "value": "3992 Sunset Ln"
      },
      "line2": {
        "value": "2373 Market Dr"
      },
      "line3": {
        "value": "7347 Oak Blvd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "34323"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.6610@example.com"
      },
      "phone_number": {
        "value": "6620404650"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "8764 Pine Dr"
      },
      "line2": {
        "value": "2331 Market St"
      },
      "line3": {
        "value": "5451 Sunset St"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "88042"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.1150@example.com"
      },
      "phone_number": {
        "value": "8471651554"
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
  "merchant_transaction_id": "mti_30a14da97aba4282b6d6f94e2382fdf2",
  "connector_transaction_id": "7742691032676882004807",
  "status": "CHARGED",
  "status_code": 201,
  "response_headers": {
    "content-length": "1125",
    "expires": "-1",
    "v-c-correlation-id": "b5df98b1-513c-4073-a79f-cb77ebb9b855",
    "content-type": "application/hal+json",
    "strict-transport-security": "max-age=31536000",
    "x-opnet-transaction-trace": "d0b8bcd4-86a9-4402-95e8-35c26c084b92-2277564-19157724",
    "pragma": "no-cache",
    "x-response-time": "132ms",
    "x-requestid": "7742691032676882004807",
    "cache-control": "no-cache, no-store, must-revalidate"
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
  "connector_transaction_id": "7742691032676882004807",
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
    "x-opnet-transaction-trace": "c6da0384-aff8-4d30-af35-f1073880e050-2349521-20331986",
    "connection": "keep-alive",
    "x-requestid": "7742691042946259904805",
    "content-type": "application/hal+json",
    "content-length": "211",
    "v-c-correlation-id": "445aa13c-2e92-41c7-8bbd-d59b3cd562a3",
    "strict-transport-security": "max-age=31536000",
    "cache-control": "no-cache, no-store, must-revalidate",
    "x-response-time": "60ms",
    "pragma": "no-cache",
    "expires": "-1"
  },
  "raw_connector_response": "***MASKED***"
}
```

</details>


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
