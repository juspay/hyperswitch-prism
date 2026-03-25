# Connector `cybersource` / Suite `void` / Scenario `void_with_amount`

- Service: `PaymentService/Void`
- PM / PMT: `-` / `-`
- Result: `PASS`

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
<summary>2. authorize(no3ds_manual_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```json
{
  "merchant_transaction_id": "mti_d8d85f1e9eb7413ab17cccb5a2ea5c80",
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
        "value": "Ethan Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Emma Wilson",
    "email": {
      "value": "sam.9335@example.com"
    },
    "id": "cust_db38b0608e784d3785c1fb4c4fb48e20",
    "phone_number": "+916353410556"
  },
  "metadata": {
    "value": "{}"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "6052 Main Dr"
      },
      "line2": {
        "value": "7918 Pine Ln"
      },
      "line3": {
        "value": "811 Oak Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "78718"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.2225@example.com"
      },
      "phone_number": {
        "value": "9965892827"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "1422 Main Ln"
      },
      "line2": {
        "value": "7466 Main Ave"
      },
      "line3": {
        "value": "2 Main Ln"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "88088"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.8601@sandbox.example.com"
      },
      "phone_number": {
        "value": "6428531417"
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
  "merchant_transaction_id": "mti_d8d85f1e9eb7413ab17cccb5a2ea5c80",
  "connector_transaction_id": "7742679172856413104807",
  "status": "AUTHORIZED",
  "status_code": 201,
  "response_headers": {
    "strict-transport-security": "max-age=31536000",
    "x-response-time": "279ms",
    "cache-control": "no-cache, no-store, must-revalidate",
    "x-opnet-transaction-trace": "d0b8bcd4-86a9-4402-95e8-35c26c084b92-2277564-18926956",
    "expires": "-1",
    "pragma": "no-cache",
    "x-requestid": "7742679172856413104807",
    "content-length": "1829",
    "v-c-correlation-id": "92cd730b-171c-46dd-9917-a33a1c87d5d6",
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
  "connector_transaction_id": "7742679172856413104807",
  "merchant_void_id": "mvi_fd5fe10cd9154da88d6dd5d277348cb1",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_order_id": "gen_457500",
  "connector_feature_data": {
    "value": "{\"disable_avs\":false,\"disable_cvn\":false}"
  },
  "cancellation_reason": "requested_by_customer"
}
```

</details>

<details>
<summary>Show Response (masked)</summary>

```json
{
  "connector_transaction_id": "7742679184386414204807",
  "status": "VOIDED",
  "status_code": 201,
  "response_headers": {
    "strict-transport-security": "max-age=31536000",
    "x-response-time": "72ms",
    "v-c-correlation-id": "fc6d8da6-36ca-467b-8c33-81555063a55b",
    "x-opnet-transaction-trace": "d0b8bcd4-86a9-4402-95e8-35c26c084b92-2277564-18927124",
    "cache-control": "no-cache, no-store, must-revalidate",
    "pragma": "no-cache",
    "content-type": "application/hal+json",
    "expires": "-1",
    "connection": "keep-alive",
    "content-length": "421",
    "x-requestid": "7742679184386414204807"
  },
  "merchant_void_id": "mvi_fd5fe10cd9154da88d6dd5d277348cb1",
  "incremental_authorization_allowed": "***MASKED***"
}
```

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
