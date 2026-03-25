# Connector `wellsfargo` / Suite `authorize` / Scenario `threeds_manual_capture_credit_card`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `credit`
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
<summary>Show Request (masked)</summary>

```json
{
  "merchant_transaction_id": "mti_ee32fab7aede40e6b3c92d3cc9d23f65",
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
        "value": "Ethan Johnson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ava Wilson",
    "email": {
      "value": "morgan.8728@testmail.io"
    },
    "id": "cust_e35dc30716cc4a868870ac342bc9de71",
    "phone_number": "+918143583059"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "1476 Market St"
      },
      "line2": {
        "value": "7831 Main Rd"
      },
      "line3": {
        "value": "5398 Pine Blvd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "71201"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.7501@testmail.io"
      },
      "phone_number": {
        "value": "9279062408"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "9588 Main Ln"
      },
      "line2": {
        "value": "6146 Pine Ln"
      },
      "line3": {
        "value": "6623 Main Rd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "41036"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.1040@testmail.io"
      },
      "phone_number": {
        "value": "4518656639"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "THREE_DS",
  "enrolled_for_3ds": true,
  "return_url": "https://example.com/payment/return",
  "webhook_url": "https://example.com/payment/webhook",
  "complete_authorize_url": "https://example.com/payment/complete",
  "order_category": "physical",
  "setup_future_usage": "ON_SESSION",
  "off_session": false,
  "description": "3DS manual capture card payment (credit)",
  "payment_channel": "ECOMMERCE",
  "test_mode": true
}
```

</details>

<details>
<summary>Show Response (masked)</summary>

```json
{
  "merchant_transaction_id": "mti_ee32fab7aede40e6b3c92d3cc9d23f65",
  "connector_transaction_id": "7742690960186880304807",
  "status": "AUTHORIZED",
  "status_code": 201,
  "response_headers": {
    "strict-transport-security": "max-age=31536000",
    "content-length": "1205",
    "x-opnet-transaction-trace": "d0b8bcd4-86a9-4402-95e8-35c26c084b92-2277564-19156373",
    "cache-control": "no-cache, no-store, must-revalidate",
    "pragma": "no-cache",
    "content-type": "application/hal+json",
    "x-requestid": "7742690960186880304807",
    "expires": "-1",
    "v-c-correlation-id": "09000e85-3f7c-4f26-9290-323f4b2d0fa8",
    "x-response-time": "88ms"
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


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
