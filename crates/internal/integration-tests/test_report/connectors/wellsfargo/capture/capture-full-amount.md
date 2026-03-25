# Connector `wellsfargo` / Suite `capture` / Scenario `capture_full_amount`

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
  "merchant_transaction_id": "mti_457c866677af489696e67e471474694d",
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
        "value": "Mia Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Wilson",
    "email": {
      "value": "sam.1016@testmail.io"
    },
    "id": "cust_3c419e9a08914fb49859020bbc2f2e1f",
    "phone_number": "+18841081050"
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
        "value": "6831 Main Ave"
      },
      "line2": {
        "value": "5931 Pine St"
      },
      "line3": {
        "value": "1731 Market Ave"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "82448"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.9093@example.com"
      },
      "phone_number": {
        "value": "5143831741"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "9602 Pine Blvd"
      },
      "line2": {
        "value": "1268 Oak Ave"
      },
      "line3": {
        "value": "9930 Market St"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "38865"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.3709@testmail.io"
      },
      "phone_number": {
        "value": "2219793660"
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
  "merchant_transaction_id": "mti_457c866677af489696e67e471474694d",
  "connector_transaction_id": "7742690970266258604805",
  "status": "AUTHORIZED",
  "status_code": 201,
  "response_headers": {
    "expires": "-1",
    "v-c-correlation-id": "3050267c-588b-4e58-a410-7d7fd0ccde6c",
    "x-response-time": "102ms",
    "content-length": "1205",
    "strict-transport-security": "max-age=31536000",
    "pragma": "no-cache",
    "x-opnet-transaction-trace": "c6da0384-aff8-4d30-af35-f1073880e050-2349521-20330677",
    "cache-control": "no-cache, no-store, must-revalidate",
    "content-type": "application/hal+json",
    "x-requestid": "7742690970266258604805"
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
  "connector_transaction_id": "7742690970266258604805",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_45f3c541ddcd48f9adc3c02a8f9a35e3"
}
```

</details>

<details>
<summary>Show Response (masked)</summary>

```json
{
  "connector_transaction_id": "7742690980776880604807",
  "status": "CHARGED",
  "status_code": 201,
  "response_headers": {
    "v-c-correlation-id": "4db46fda-3848-4d0c-b5c1-db9784a0ebd7",
    "x-response-time": "56ms",
    "expires": "-1",
    "cache-control": "no-cache, no-store, must-revalidate",
    "content-type": "application/hal+json",
    "x-opnet-transaction-trace": "d0b8bcd4-86a9-4402-95e8-35c26c084b92-2277564-19156791",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000",
    "x-requestid": "7742690980776880604807",
    "content-length": "438",
    "connection": "keep-alive"
  },
  "merchant_capture_id": "mci_45f3c541ddcd48f9adc3c02a8f9a35e3",
  "incremental_authorization_allowed": "***MASKED***"
}
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
