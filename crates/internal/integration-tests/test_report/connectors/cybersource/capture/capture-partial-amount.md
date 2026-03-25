# Connector `cybersource` / Suite `capture` / Scenario `capture_partial_amount`

- Service: `PaymentService/Capture`
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
  "merchant_transaction_id": "mti_c07386c7002c431d8a58efd5e23ba824",
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
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Taylor",
    "email": {
      "value": "casey.1953@sandbox.example.com"
    },
    "id": "cust_baa260128cbe41e280f02e0de47c1b10",
    "phone_number": "+918236830380"
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
        "value": "Wilson"
      },
      "line1": {
        "value": "6223 Lake Ln"
      },
      "line2": {
        "value": "1749 Main St"
      },
      "line3": {
        "value": "8695 Pine Ln"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "12622"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.3832@example.com"
      },
      "phone_number": {
        "value": "1735374673"
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
        "value": "4350 Oak Ave"
      },
      "line2": {
        "value": "6982 Market Ave"
      },
      "line3": {
        "value": "6004 Sunset St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "41185"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.4810@example.com"
      },
      "phone_number": {
        "value": "6853694748"
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
  "merchant_transaction_id": "mti_c07386c7002c431d8a58efd5e23ba824",
  "connector_transaction_id": "7742679046596399704807",
  "status": "AUTHORIZED",
  "status_code": 201,
  "response_headers": {
    "x-opnet-transaction-trace": "d0b8bcd4-86a9-4402-95e8-35c26c084b92-2277564-18924498",
    "strict-transport-security": "max-age=31536000",
    "cache-control": "no-cache, no-store, must-revalidate",
    "x-response-time": "308ms",
    "expires": "-1",
    "pragma": "no-cache",
    "content-type": "application/hal+json",
    "content-length": "1829",
    "x-requestid": "7742679046596399704807",
    "v-c-correlation-id": "23e7e881-0149-46ea-843c-f140157453af"
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
  "connector_transaction_id": "7742679046596399704807",
  "amount_to_capture": {
    "minor_amount": 3000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_cd27c726abc94be98bbdbb7d025e1895",
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
  "connector_transaction_id": "7742679058776401504807",
  "status": "PENDING",
  "status_code": 201,
  "response_headers": {
    "content-length": "438",
    "x-requestid": "7742679058776401504807",
    "cache-control": "no-cache, no-store, must-revalidate",
    "strict-transport-security": "max-age=31536000",
    "content-type": "application/hal+json",
    "x-response-time": "77ms",
    "x-opnet-transaction-trace": "d0b8bcd4-86a9-4402-95e8-35c26c084b92-2277564-18924769",
    "v-c-correlation-id": "c119388c-ede6-4b38-ae4b-8693495e9035",
    "pragma": "no-cache",
    "expires": "-1",
    "connection": "keep-alive"
  },
  "merchant_capture_id": "mci_cd27c726abc94be98bbdbb7d025e1895",
  "incremental_authorization_allowed": "***MASKED***"
}
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
