# Connector `wellsfargo` / Suite `capture` / Scenario `capture_partial_amount`

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
  "merchant_transaction_id": "mti_f885f6a98fd148e488c2b32ddb6edc7e",
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
        "value": "Mia Brown"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ethan Johnson",
    "email": {
      "value": "alex.3011@sandbox.example.com"
    },
    "id": "cust_bbcc002b001042168404d2bf25deef65",
    "phone_number": "+16591396140"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "5268 Pine Blvd"
      },
      "line2": {
        "value": "6618 Oak Ln"
      },
      "line3": {
        "value": "9872 Oak Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "54680"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.2128@testmail.io"
      },
      "phone_number": {
        "value": "8727303014"
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
        "value": "2853 Lake Dr"
      },
      "line2": {
        "value": "5839 Lake Blvd"
      },
      "line3": {
        "value": "8689 Lake Ave"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "40544"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.1231@testmail.io"
      },
      "phone_number": {
        "value": "1843332070"
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
  "merchant_transaction_id": "mti_f885f6a98fd148e488c2b32ddb6edc7e",
  "connector_transaction_id": "7742690991796259204805",
  "status": "AUTHORIZED",
  "status_code": 201,
  "response_headers": {
    "content-type": "application/hal+json",
    "x-opnet-transaction-trace": "c6da0384-aff8-4d30-af35-f1073880e050-2349521-20331078",
    "x-requestid": "7742690991796259204805",
    "strict-transport-security": "max-age=31536000",
    "cache-control": "no-cache, no-store, must-revalidate",
    "pragma": "no-cache",
    "x-response-time": "104ms",
    "content-length": "1205",
    "expires": "-1",
    "v-c-correlation-id": "ea1be72c-b7b8-4532-9208-c8384cdeb91a"
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
  "connector_transaction_id": "7742690991796259204805",
  "amount_to_capture": {
    "minor_amount": 3000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_2ad9ee2b034a40d5a9f44d478e4129d9"
}
```

</details>

<details>
<summary>Show Response (masked)</summary>

```json
{
  "connector_transaction_id": "7742691001666259304805",
  "status": "CHARGED",
  "status_code": 201,
  "response_headers": {
    "x-opnet-transaction-trace": "c6da0384-aff8-4d30-af35-f1073880e050-2349521-20331252",
    "strict-transport-security": "max-age=31536000",
    "pragma": "no-cache",
    "cache-control": "no-cache, no-store, must-revalidate",
    "x-requestid": "7742691001666259304805",
    "connection": "keep-alive",
    "v-c-correlation-id": "abac653d-42c0-48d7-8ea1-4f75c29a7cf6",
    "content-type": "application/hal+json",
    "expires": "-1",
    "x-response-time": "51ms",
    "content-length": "438"
  },
  "merchant_capture_id": "mci_2ad9ee2b034a40d5a9f44d478e4129d9",
  "incremental_authorization_allowed": "***MASKED***"
}
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
