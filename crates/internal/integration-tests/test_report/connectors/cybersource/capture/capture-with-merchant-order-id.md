# Connector `cybersource` / Suite `capture` / Scenario `capture_with_merchant_order_id`

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
  "merchant_transaction_id": "mti_1766d2ec94814ecfa857786088eed910",
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
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Johnson",
    "email": {
      "value": "sam.8451@testmail.io"
    },
    "id": "cust_6235ed28ab85447fa920d11f1727c7a9",
    "phone_number": "+446834740314"
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
        "value": "Miller"
      },
      "line1": {
        "value": "5884 Main Rd"
      },
      "line2": {
        "value": "6300 Pine Blvd"
      },
      "line3": {
        "value": "7532 Sunset Ln"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "69714"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.9973@testmail.io"
      },
      "phone_number": {
        "value": "8095393814"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "8893 Market Rd"
      },
      "line2": {
        "value": "7304 Oak St"
      },
      "line3": {
        "value": "9541 Lake Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "42581"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.2639@example.com"
      },
      "phone_number": {
        "value": "7762363565"
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
  "merchant_transaction_id": "mti_1766d2ec94814ecfa857786088eed910",
  "connector_transaction_id": "7742679069546093604806",
  "status": "AUTHORIZED",
  "status_code": 201,
  "response_headers": {
    "pragma": "no-cache",
    "x-response-time": "313ms",
    "x-opnet-transaction-trace": "687b1be9-a770-4c1b-984d-e09c8a4b9f43-2283790-19580657",
    "content-type": "application/hal+json",
    "x-requestid": "7742679069546093604806",
    "content-length": "1829",
    "v-c-correlation-id": "55b35f08-92ab-4ef1-9d16-a2eff33dc1f2",
    "expires": "-1",
    "cache-control": "no-cache, no-store, must-revalidate",
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
  "connector_transaction_id": "7742679069546093604806",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_265fc47493cc400c8826528cca951aab",
  "merchant_order_id": "gen_300128",
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
  "connector_transaction_id": "7742679082406403804807",
  "status": "PENDING",
  "status_code": 201,
  "response_headers": {
    "content-length": "438",
    "expires": "-1",
    "cache-control": "no-cache, no-store, must-revalidate",
    "x-response-time": "69ms",
    "connection": "keep-alive",
    "v-c-correlation-id": "e29749f7-2a69-4496-9faa-6025e91bdaf6",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000",
    "content-type": "application/hal+json",
    "x-requestid": "7742679082406403804807",
    "x-opnet-transaction-trace": "d0b8bcd4-86a9-4402-95e8-35c26c084b92-2277564-18925362"
  },
  "merchant_capture_id": "mci_265fc47493cc400c8826528cca951aab",
  "incremental_authorization_allowed": "***MASKED***"
}
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
