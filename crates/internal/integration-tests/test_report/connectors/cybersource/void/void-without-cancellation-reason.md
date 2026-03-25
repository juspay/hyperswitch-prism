# Connector `cybersource` / Suite `void` / Scenario `void_without_cancellation_reason`

- Service: `PaymentService/Void`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
sdk call failed: sdk request transformer failed for 'void/void_without_cancellation_reason': Missing required field: currency (code: BAD_REQUEST)
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
<summary>2. authorize(no3ds_manual_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```json
{
  "merchant_transaction_id": "mti_21a491bbc7f44daa8a9cc44c721c0b0b",
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
        "value": "Noah Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Mia Brown",
    "email": {
      "value": "jordan.1668@example.com"
    },
    "id": "cust_0e58420bbf9d415b8b4e55c245934c80",
    "phone_number": "+449943957725"
  },
  "metadata": {
    "value": "{}"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "6007 Market Dr"
      },
      "line2": {
        "value": "574 Pine Dr"
      },
      "line3": {
        "value": "8022 Oak Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "72133"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.4101@sandbox.example.com"
      },
      "phone_number": {
        "value": "8640381868"
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
        "value": "6583 Main Dr"
      },
      "line2": {
        "value": "2849 Oak Ave"
      },
      "line3": {
        "value": "4135 Sunset St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "53497"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.1047@example.com"
      },
      "phone_number": {
        "value": "6713679743"
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
  "merchant_transaction_id": "mti_21a491bbc7f44daa8a9cc44c721c0b0b",
  "connector_transaction_id": "7742679194986104504806",
  "status": "AUTHORIZED",
  "status_code": 201,
  "response_headers": {
    "x-opnet-transaction-trace": "687b1be9-a770-4c1b-984d-e09c8a4b9f43-2283790-19582789",
    "strict-transport-security": "max-age=31536000",
    "expires": "-1",
    "pragma": "no-cache",
    "content-type": "application/hal+json",
    "x-requestid": "7742679194986104504806",
    "v-c-correlation-id": "e215b2f6-10c3-49a9-926d-0dc76f6ab004",
    "content-length": "1829",
    "cache-control": "no-cache, no-store, must-revalidate",
    "x-response-time": "273ms"
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

_Request trace not available._

</details>

<details>
<summary>Show Response (masked)</summary>

_Response trace not available._

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
