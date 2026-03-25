# Connector `wellsfargo` / Suite `authorize` / Scenario `no3ds_manual_capture_debit_card`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `debit`
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
  "merchant_transaction_id": "mti_940081d85c8e40f2946c93c20087e1eb",
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
        "value": "Noah Smith"
      },
      "card_type": "debit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Noah Wilson",
    "email": {
      "value": "alex.6406@sandbox.example.com"
    },
    "id": "cust_8d6af43333434fe68e088697361f065a",
    "phone_number": "+911187595663"
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
        "value": "7465 Lake St"
      },
      "line2": {
        "value": "8280 Market Blvd"
      },
      "line3": {
        "value": "8155 Lake Ave"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "14932"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.9296@example.com"
      },
      "phone_number": {
        "value": "3949404336"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "2086 Lake Blvd"
      },
      "line2": {
        "value": "1615 Market Ave"
      },
      "line3": {
        "value": "9351 Lake Rd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "85535"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.8769@example.com"
      },
      "phone_number": {
        "value": "1770162677"
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
  "description": "No3DS manual capture card payment (debit)",
  "payment_channel": "ECOMMERCE",
  "test_mode": true
}
```

</details>

<details>
<summary>Show Response (masked)</summary>

```json
{
  "merchant_transaction_id": "mti_940081d85c8e40f2946c93c20087e1eb",
  "connector_transaction_id": "7742690949636880004807",
  "status": "AUTHORIZED",
  "status_code": 201,
  "response_headers": {
    "cache-control": "no-cache, no-store, must-revalidate",
    "x-requestid": "7742690949636880004807",
    "pragma": "no-cache",
    "x-response-time": "89ms",
    "content-length": "1205",
    "expires": "-1",
    "content-type": "application/hal+json",
    "strict-transport-security": "max-age=31536000",
    "v-c-correlation-id": "19bca2d3-45d6-4a86-b0e8-eb438bd7a06a",
    "x-opnet-transaction-trace": "d0b8bcd4-86a9-4402-95e8-35c26c084b92-2277564-19156182"
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
