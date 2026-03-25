# Connector `cybersource` / Suite `capture` / Scenario `capture_full_amount`

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
  "merchant_transaction_id": "mti_579a5da671e84f848004be115e2d20ac",
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
        "value": "Ava Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Mia Brown",
    "email": {
      "value": "casey.9408@sandbox.example.com"
    },
    "id": "cust_ed10a0f0ea2644eb878dc1d277d0a58d",
    "phone_number": "+11710324060"
  },
  "metadata": {
    "value": "{}"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "5504 Sunset Rd"
      },
      "line2": {
        "value": "9478 Oak St"
      },
      "line3": {
        "value": "3315 Sunset Ln"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "52460"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.1346@example.com"
      },
      "phone_number": {
        "value": "5388047055"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "584 Sunset Ln"
      },
      "line2": {
        "value": "4912 Oak Dr"
      },
      "line3": {
        "value": "4511 Main Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "70262"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.5125@testmail.io"
      },
      "phone_number": {
        "value": "2178906781"
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
  "merchant_transaction_id": "mti_579a5da671e84f848004be115e2d20ac",
  "connector_transaction_id": "7742679023526771504805",
  "status": "AUTHORIZED",
  "status_code": 201,
  "response_headers": {
    "content-type": "application/hal+json",
    "pragma": "no-cache",
    "x-opnet-transaction-trace": "c6da0384-aff8-4d30-af35-f1073880e050-2349521-20096943",
    "content-length": "1863",
    "cache-control": "no-cache, no-store, must-revalidate",
    "v-c-correlation-id": "a3beeb48-4c55-4d75-bf7e-7caf2722c66b",
    "strict-transport-security": "max-age=31536000",
    "x-requestid": "7742679023526771504805",
    "x-response-time": "281ms",
    "expires": "-1"
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
  "connector_transaction_id": "7742679023526771504805",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_8fd0bf2ec3a84c689a16efa0039c65c0",
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
  "connector_transaction_id": "7742679035966089804806",
  "status": "PENDING",
  "status_code": 201,
  "response_headers": {
    "content-type": "application/hal+json",
    "content-length": "438",
    "cache-control": "no-cache, no-store, must-revalidate",
    "pragma": "no-cache",
    "x-opnet-transaction-trace": "687b1be9-a770-4c1b-984d-e09c8a4b9f43-2283790-19579799",
    "expires": "-1",
    "x-response-time": "58ms",
    "connection": "keep-alive",
    "v-c-correlation-id": "ba57b3c8-4450-454a-b8b3-004747269ffe",
    "x-requestid": "7742679035966089804806",
    "strict-transport-security": "max-age=31536000"
  },
  "merchant_capture_id": "mci_8fd0bf2ec3a84c689a16efa0039c65c0",
  "incremental_authorization_allowed": "***MASKED***"
}
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
