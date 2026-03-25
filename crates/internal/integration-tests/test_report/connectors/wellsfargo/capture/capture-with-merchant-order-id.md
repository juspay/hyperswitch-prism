# Connector `wellsfargo` / Suite `capture` / Scenario `capture_with_merchant_order_id`

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
  "merchant_transaction_id": "mti_fc6094dc10864df3bdc739e3b1a635f2",
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
        "value": "Liam Johnson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Noah Miller",
    "email": {
      "value": "jordan.1992@sandbox.example.com"
    },
    "id": "cust_dce0ff4b297f44c3a69107f2c3aede24",
    "phone_number": "+919169387276"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "3044 Pine Rd"
      },
      "line2": {
        "value": "7898 Lake Ave"
      },
      "line3": {
        "value": "9026 Market Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "53996"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.4523@example.com"
      },
      "phone_number": {
        "value": "6840444670"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "7941 Oak Ave"
      },
      "line2": {
        "value": "7547 Sunset St"
      },
      "line3": {
        "value": "1578 Pine Ave"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "96601"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.3036@example.com"
      },
      "phone_number": {
        "value": "9689261827"
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
  "merchant_transaction_id": "mti_fc6094dc10864df3bdc739e3b1a635f2",
  "connector_transaction_id": "7742691011396881504807",
  "status": "AUTHORIZED",
  "status_code": 201,
  "response_headers": {
    "v-c-correlation-id": "811b1a47-d97b-4d07-ac94-a9a7172fbe94",
    "x-response-time": "91ms",
    "content-length": "1205",
    "x-requestid": "7742691011396881504807",
    "cache-control": "no-cache, no-store, must-revalidate",
    "pragma": "no-cache",
    "expires": "-1",
    "content-type": "application/hal+json",
    "strict-transport-security": "max-age=31536000",
    "x-opnet-transaction-trace": "d0b8bcd4-86a9-4402-95e8-35c26c084b92-2277564-19157339"
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
  "connector_transaction_id": "7742691011396881504807",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_4c06110938294de5ab0833919192d413",
  "merchant_order_id": "gen_126807"
}
```

</details>

<details>
<summary>Show Response (masked)</summary>

```json
{
  "connector_transaction_id": "7742691022066577004806",
  "status": "CHARGED",
  "status_code": 201,
  "response_headers": {
    "pragma": "no-cache",
    "v-c-correlation-id": "d81130a2-e33b-4d46-bbd0-e6ab3f5885ff",
    "expires": "-1",
    "strict-transport-security": "max-age=31536000",
    "x-response-time": "59ms",
    "x-requestid": "7742691022066577004806",
    "x-opnet-transaction-trace": "687b1be9-a770-4c1b-984d-e09c8a4b9f43-2283790-19812268",
    "cache-control": "no-cache, no-store, must-revalidate",
    "content-length": "438",
    "content-type": "application/hal+json",
    "connection": "keep-alive"
  },
  "merchant_capture_id": "mci_4c06110938294de5ab0833919192d413",
  "incremental_authorization_allowed": "***MASKED***"
}
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
