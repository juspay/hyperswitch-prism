# Connector `wellsfargo` / Suite `void` / Scenario `void_with_amount`

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
  "merchant_transaction_id": "mti_0f72da4e9b8c4241b32c2a04eb783580",
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
    "name": "Ava Wilson",
    "email": {
      "value": "riley.3229@sandbox.example.com"
    },
    "id": "cust_0f30ee3452114c4b8c11a2d8e88084ff",
    "phone_number": "+19632524542"
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
        "value": "360 Pine Ave"
      },
      "line2": {
        "value": "5194 Main Ln"
      },
      "line3": {
        "value": "8347 Oak Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "59555"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.4426@example.com"
      },
      "phone_number": {
        "value": "3929293229"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "2224 Market Dr"
      },
      "line2": {
        "value": "5185 Oak Dr"
      },
      "line3": {
        "value": "8479 Market Rd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "32195"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.4879@testmail.io"
      },
      "phone_number": {
        "value": "4118719818"
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
  "merchant_transaction_id": "mti_0f72da4e9b8c4241b32c2a04eb783580",
  "connector_transaction_id": "7742691147726579704806",
  "status": "AUTHORIZED",
  "status_code": 201,
  "response_headers": {
    "content-type": "application/hal+json",
    "strict-transport-security": "max-age=31536000",
    "content-length": "1205",
    "v-c-correlation-id": "6da6f761-fed3-427f-b728-73f93674bcd4",
    "x-opnet-transaction-trace": "687b1be9-a770-4c1b-984d-e09c8a4b9f43-2283790-19814081",
    "x-response-time": "103ms",
    "cache-control": "no-cache, no-store, must-revalidate",
    "pragma": "no-cache",
    "x-requestid": "7742691147726579704806",
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
  "connector_transaction_id": "7742691147726579704806",
  "merchant_void_id": "mvi_86d22953cc7c42c99900fcdf68c067b5",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_order_id": "gen_634912",
  "cancellation_reason": "requested_by_customer"
}
```

</details>

<details>
<summary>Show Response (masked)</summary>

```json
{
  "connector_transaction_id": "7742691157666580004806",
  "status": "VOIDED",
  "status_code": 201,
  "response_headers": {
    "x-response-time": "78ms",
    "cache-control": "no-cache, no-store, must-revalidate",
    "strict-transport-security": "max-age=31536000",
    "x-opnet-transaction-trace": "687b1be9-a770-4c1b-984d-e09c8a4b9f43-2283790-19814208",
    "expires": "-1",
    "connection": "keep-alive",
    "content-length": "421",
    "x-requestid": "7742691157666580004806",
    "pragma": "no-cache",
    "content-type": "application/hal+json",
    "v-c-correlation-id": "0d3001ec-deb6-4a4f-aa32-5430e41717f3"
  },
  "merchant_void_id": "mvi_86d22953cc7c42c99900fcdf68c067b5",
  "incremental_authorization_allowed": "***MASKED***"
}
```

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
